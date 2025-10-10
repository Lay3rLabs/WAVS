use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::Log;
use slotmap::Key;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::subsystems::trigger::streams::evm_stream::client::rpc_types::outbound::SubscribeParams;

use super::{
    channels::SubscriptionChannels,
    connection::{ConnectionData, ConnectionState},
    rpc_types::{
        id::{RpcId, RpcIds, RpcRequestKind},
        inbound::{RpcInbound, RpcResponse, RpcSubscriptionEvent},
        outbound::RpcRequest,
    },
};

pub struct Subscriptions {
    handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    inner: Arc<SubscriptionsInner>,
}

impl Subscriptions {
    pub fn new(rpc_ids: RpcIds, channels: SubscriptionChannels) -> Self {
        let SubscriptionChannels {
            mut subscription_block_height_tx,
            mut subscription_log_tx,
            mut subscription_new_pending_transaction_tx,
            connection_send_rpc_tx,
            mut connection_state_rx,
            mut connection_data_rx,
        } = channels;

        let inner = Arc::new(SubscriptionsInner::new(rpc_ids, connection_send_rpc_tx));

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let handle = tokio::spawn({
            let inner = inner.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => {
                            tracing::info!("EVM: shutdown requested, exiting subscription loop");
                            break;
                        }

                        Some(msg) = connection_data_rx.recv() => {
                            match msg {
                                ConnectionData::Text(text) => {
                                    match serde_json::from_str::<RpcInbound>(&text) {
                                        Ok(inbound) => {
                                            match inbound {
                                                RpcInbound::Response {id, result} => {
                                                    match result {
                                                        Ok(response) => {
                                                            inner.on_received_rpc_response(id, response);
                                                        }
                                                        Err(err) => {
                                                            tracing::error!("EVM: RPC error for id {}: {:?}", id.data().as_ffi(), err);
                                                        }
                                                    }
                                                },
                                                RpcInbound::Subscription{id, result } => {
                                                    match result {
                                                        Ok(event) => {
                                                            inner.on_received_subscription_event(
                                                                &mut subscription_block_height_tx,
                                                                &mut subscription_log_tx,
                                                                &mut subscription_new_pending_transaction_tx,
                                                                id,
                                                                event
                                                            );
                                                        }
                                                        Err(err) => {
                                                            tracing::error!("EVM: Subscription error for id {}: {:?}", id, err);
                                                        }
                                                    }
                                                },

                                            }
                                        },
                                        Err(e) => {
                                            tracing::error!("EVM: failed to parse RPC response: {}", e);
                                        }
                                    }
                                },
                                ConnectionData::Binary(bin) => {
                                    tracing::debug!("EVM: received binary message: {:x?}", bin);
                                    // ignore binary messages for now
                                },
                            }
                        }
                        Some(state) = connection_state_rx.recv() => {
                            match state {
                                ConnectionState::Connected(_endpoint) => {
                                    inner.set_is_connected(true);
                                },
                                ConnectionState::Disconnected => {
                                    inner.set_is_connected(false);
                                },
                            }
                        }
                    }
                }
            }
        });

        Self {
            handle: Some(handle),
            shutdown_tx: Some(shutdown_tx),
            inner,
        }
    }

    pub fn toggle_block_height(&self, value: bool) {
        self.inner.toggle_block_height(value);
    }

    pub fn enable_logs(&self, addresses: Vec<Address>, events: Vec<B256>) {
        self.inner.enable_logs(addresses, events);
    }

    pub fn disable_logs(&self, addresses: &[Address], events: &[B256]) {
        self.inner.disable_logs(addresses, events);
    }

    pub fn disable_all_logs(&self) {
        self.inner.disable_all_logs();
    }

    pub fn toggle_pending_transactions(&self, value: bool) {
        self.inner.toggle_pending_transactions(value);
    }

    pub fn any_active_rpcs_in_flight(&self) -> bool {
        self.inner.rpc_ids_in_flight.any_active_in_flight()
    }

    pub fn is_connected(&self) -> bool {
        self.inner
            ._is_connected
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn active_subscriptions(&self) -> HashMap<String, SubscriptionKind> {
        self.inner.ids._lookup.read().unwrap().clone()
    }
}

impl Drop for Subscriptions {
    fn drop(&mut self) {
        tracing::info!("EVM: subscription dropped");
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(mut handle) = self.handle.take() {
            tokio::spawn(async move {
                if tokio::time::timeout(Duration::from_millis(500), &mut handle)
                    .await
                    .is_err()
                {
                    tracing::warn!("EVM: subscription loop did not shut down in time, aborting");
                    handle.abort();
                }
            });
        }
    }
}

struct SubscriptionsInner {
    _blocks: AtomicBool,
    _logs: std::sync::RwLock<Option<LogFilter>>,
    _pending_transactions: AtomicBool,
    // not really a subscription, but used to track connection state
    _is_connected: AtomicBool,
    ids: SubscriptionIds,
    rpc_ids: RpcIds,
    rpc_ids_in_flight: RpcIdsInFlight,
    _connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>,
}

impl SubscriptionsInner {
    pub fn new(
        rpc_ids: RpcIds,
        connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>,
    ) -> Self {
        Self {
            _blocks: AtomicBool::new(false),
            _logs: std::sync::RwLock::new(None),
            _pending_transactions: AtomicBool::new(false),
            _is_connected: AtomicBool::new(false),
            ids: SubscriptionIds::default(),
            rpc_ids,
            rpc_ids_in_flight: RpcIdsInFlight::default(),
            _connection_send_rpc_tx: connection_send_rpc_tx,
        }
    }

    pub fn toggle_block_height(&self, toggle: bool) {
        self._blocks
            .store(toggle, std::sync::atomic::Ordering::SeqCst);

        if !toggle {
            self.unsubscribe(UnsubscribeKind::NewHeads);
        } else {
            // only need to resubscribe if turning on since we only have one kind of "newHeads" subscription atm
            self.resubscribe();
        }
    }

    pub fn enable_logs(&self, address: Vec<Address>, topics: Vec<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            let lock = lock.get_or_insert_default();

            for address in address {
                lock.addresses.insert(address);
            }

            for topic in topics {
                lock.topics.insert(topic);
            }
        }
        self.unsubscribe(UnsubscribeKind::AllLogs);

        // logs is different, needs to resubscribe since different filters are different subscriptions
        self.resubscribe();
    }

    // disable specific log filters, if no filters remain, it will unsubscribe from all logs
    // if you want to instead subscribe to all logs, call `enable_logs` with empty vecs
    pub fn disable_logs(&self, addresses: &[Address], topics: &[B256]) {
        {
            let mut lock = self._logs.write().unwrap();

            match lock.as_mut() {
                None => {} // nothing to do
                Some(logs) => {
                    for address in addresses {
                        logs.addresses.remove(address);
                    }

                    for topic in topics {
                        logs.topics.remove(topic);
                    }

                    if logs.addresses.is_empty() && logs.topics.is_empty() {
                        tracing::warn!("No more filters remaining, disabling *all* log filters. If you meant to remove all the filters in order to subractively get a catch-all, call `enable_logs()` with empty vecs instead");
                        *lock = None;
                    }
                }
            }
        }
        self.unsubscribe(UnsubscribeKind::AllLogs);
        self.resubscribe();
    }

    pub fn disable_all_logs(&self) {
        *self._logs.write().unwrap() = None;
        self.unsubscribe(UnsubscribeKind::AllLogs);
        // no need to resubscribe
    }

    pub fn toggle_pending_transactions(&self, toggle: bool) {
        self._pending_transactions
            .store(toggle, std::sync::atomic::Ordering::SeqCst);
        if !toggle {
            self.unsubscribe(UnsubscribeKind::NewPendingTransactions);
        } else {
            // only need to resubscribe if turning on since we only have one kind of "pending txs" subscription atm
            self.resubscribe();
        }
    }

    pub fn set_is_connected(&self, value: bool) {
        self._is_connected
            .store(value, std::sync::atomic::Ordering::SeqCst);

        if !value {
            self.ids.clear();
            self.rpc_ids_in_flight.clear();
        } else {
            self.resubscribe();
        }
    }

    // all requests must go through here so we can track in-flight requests
    fn send_rpc(
        &self,
        req: RpcRequest,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<RpcRequest>> {
        match &req {
            RpcRequest::Subscribe { id, params } => match params {
                SubscribeParams::NewHeads => {
                    tracing::info!(
                        "EVM: sending newHeads subscription request (rpc id {})",
                        id.data().as_ffi()
                    );
                }
                SubscribeParams::Logs { addresses, topics } => {
                    tracing::info!(
                        "EVM: sending logs subscription request (rpc id {}) with {} addresses and {} topics",
                        id.data().as_ffi(),
                        addresses.len(),
                        topics.len()
                    );
                    tracing::debug!(
                        "EVM: logs subscription request (rpc id {}) details: addresses={:?}, topics={:?}",
                        id.data().as_ffi(),
                        addresses,
                        topics
                    );
                }
                SubscribeParams::NewPendingTransactions => {
                    tracing::info!(
                        "EVM: sending newPendingTransactions subscription request (rpc id {})",
                        id.data().as_ffi()
                    );
                }
            },
            RpcRequest::Unsubscribe {
                id,
                subscription_id,
            } => {
                tracing::info!(
                    "EVM: sending unsubscribe request (rpc id {}) for subscription id {}",
                    id.data().as_ffi(),
                    subscription_id
                );
            }
        }

        // this should always be Some here, but better safe than sorry
        if let Some(kind) = self.rpc_ids.kind(req.id()) {
            self.rpc_ids_in_flight.insert(req.id(), kind);
            self._connection_send_rpc_tx.send(req)?;
        } else {
            tracing::warn!(
                "couldn't get in-flight kind for rpc id {}",
                req.id().data().as_ffi()
            );
        }

        Ok(())
    }

    fn on_received_rpc_response(&self, id: RpcId, response: RpcResponse) {
        let removed_rpc_in_flight_state = self.rpc_ids_in_flight.remove(&id);
        // since we clear the rpc ids (and sub ids) on disconnect,
        // we can be sure that any new subscription id we get is for this connection
        let kind = match self.rpc_ids.kind(id) {
            Some(kind) => kind,
            None => {
                tracing::warn!(
                    "EVM: received response for unknown RPC id {}",
                    id.data().as_ffi()
                );
                return;
            }
        };
        match response {
            RpcResponse::NewSubscription { subscription_id } => {
                // if this rpc id was marked to unsubscribe on landing, do so now
                if matches!(
                    removed_rpc_in_flight_state,
                    Some(RpcFlightState::UnsubscribeOnLand)
                ) {
                    if let Err(e) = self.send_rpc(RpcRequest::unsubscribe(
                        &self.rpc_ids,
                        subscription_id.clone(),
                    )) {
                        tracing::error!(
                            "EVM: failed to send unsubscribe request for subscription id {}: {}",
                            subscription_id,
                            e
                        );
                    }
                    return; // we are done here, don't track this subscription
                }

                match SubscriptionKind::try_from(kind) {
                    Ok(kind) => {
                        self.ids.insert(subscription_id, kind);
                    }
                    Err(e) => {
                        tracing::error!("EVM: received newSubscription response for unknown subscription (rpc_id {}, subscription_id {}): {}", id.data().as_ffi(), subscription_id, e);
                    }
                }
            }
            RpcResponse::UnsubscribeAck(success) => {
                if success {
                    match kind {
                        RpcRequestKind::Unsubscribe { subscription_id } => {
                            tracing::info!(
                                "EVM: unsubscribed from subscription id {}",
                                subscription_id
                            );
                            self.ids.remove(&subscription_id);
                        }
                        _ => {
                            tracing::error!(
                                "EVM: received unsubscribeAck for non-unsubscribe request id {}",
                                id.data().as_ffi()
                            );
                        }
                    }
                } else {
                    match kind {
                        RpcRequestKind::Unsubscribe { subscription_id } => {
                            tracing::warn!(
                                "EVM: failed to unsubscribe from subscription id {}",
                                subscription_id
                            );
                        }
                        _ => {
                            tracing::error!(
                                "EVM: received unsubscribeAck for non-unsubscribe request id {}",
                                id.data().as_ffi()
                            );
                        }
                    }
                }
            }
            RpcResponse::Other(value) => {
                tracing::warn!(
                    "EVM: received unexpected RPC response for id {}: {:?}",
                    id.data().as_ffi(),
                    value
                );
            }
        }
    }

    fn on_received_subscription_event(
        &self,
        subscription_block_height_tx: &mut tokio::sync::mpsc::UnboundedSender<u64>,
        subscription_log_tx: &mut tokio::sync::mpsc::UnboundedSender<Log>,
        subscription_new_pending_transaction_tx: &mut tokio::sync::mpsc::UnboundedSender<B256>,
        subscription_id: String,
        event: RpcSubscriptionEvent,
    ) {
        match event {
            RpcSubscriptionEvent::NewHeads(header) => {
                if !self.ids.compare(&subscription_id, |kind| {
                    matches!(kind, SubscriptionKind::NewHeads)
                }) {
                    tracing::warn!(
                        "EVM: received newHeads event for unknown subscription id {}",
                        subscription_id
                    );
                } else if let Err(e) = subscription_block_height_tx.send(header.number) {
                    tracing::error!("EVM: failed to send new block height: {}", e);
                }
            }
            RpcSubscriptionEvent::Logs(log) => {
                tracing::info!(
                    "EVM: received log event for subscription id {}",
                    subscription_id
                );
                tracing::debug!(
                    "EVM: log event details: address={:?}, topics={:?}",
                    log.address(),
                    log.topics()
                );

                if !self.ids.compare(&subscription_id, |kind| {
                    matches!(kind, SubscriptionKind::Logs { .. })
                }) {
                    tracing::warn!(
                        "EVM: received logs event for unknown subscription id {}",
                        subscription_id
                    );
                } else {
                    tracing::info!("EVM: forwarding log event to channel");
                    if let Err(e) = subscription_log_tx.send(log) {
                        tracing::error!("EVM: failed to send log: {}", e);
                    } else {
                        tracing::info!("EVM: successfully sent log event to channel");
                    }
                }
            }
            RpcSubscriptionEvent::NewPendingTransaction(tx) => {
                if !self.ids.compare(&subscription_id, |kind| {
                    matches!(kind, SubscriptionKind::NewPendingTransactions)
                }) {
                    tracing::warn!(
                        "EVM: received newPendingTransaction event for unknown subscription id {}",
                        subscription_id
                    );
                }

                if let Err(e) = subscription_new_pending_transaction_tx.send(tx) {
                    tracing::error!("EVM: failed to send new pending transaction: {}", e);
                }
            }
        }
    }

    fn unsubscribe(&self, kind: UnsubscribeKind) {
        let ids = self.ids.list_by_unsubscribe(kind);

        // mark the rpcs in flight to unsubscribe when they land
        self.rpc_ids_in_flight.set_unsubscribe(kind);

        // send the unsubscribe request for the active subscriptions
        // will actually unsubscribe when the ack response lands
        for id in ids {
            if let Err(e) = self.send_rpc(RpcRequest::unsubscribe(&self.rpc_ids, id.clone())) {
                tracing::error!(
                    "EVM: failed to send unsubscribe request for subscription id {}: {}",
                    id,
                    e
                );
            }
        }
    }

    fn resubscribe(&self) {
        // exit early if not connected
        if !self._is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        // blocks/newHeads
        if self._blocks.load(std::sync::atomic::Ordering::SeqCst) {
            if !self
                .rpc_ids_in_flight
                .will_subscribe(RpcRequestKind::SubscribeNewHeads)
            {
                if let Err(e) = self.send_rpc(RpcRequest::new_heads(&self.rpc_ids)) {
                    tracing::error!("EVM: failed to send newHeads subscription request: {}", e);
                }
            } else {
                tracing::info!("EVM: already have newHeads subscription or request in flight, not sending another");
            }
        }

        // logs
        match self._logs.read().unwrap().clone() {
            None => {} // no logs to subscribe to
            Some(LogFilter { addresses, topics }) => {
                // logs is a bit tricky, the test is against the specific log filter, not just the high-level kind
                // because we can have multiple different log filters active at once while they are still unsubscribed
                if !self
                    .rpc_ids_in_flight
                    .will_subscribe(RpcRequestKind::SubscribeLogs {
                        addresses: addresses.clone(),
                        topics: topics.clone(),
                    })
                {
                    if let Err(e) =
                        self.send_rpc(RpcRequest::logs(&self.rpc_ids, addresses, topics))
                    {
                        tracing::error!("EVM: failed to send logs subscription request: {}", e);
                    }
                } else {
                    tracing::info!("EVM: already have logs subscription or request in flight for this filter, not sending another");
                }
            }
        }

        // pending transactions, similar to blocks/newHeads
        if self
            ._pending_transactions
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            if !self
                .rpc_ids_in_flight
                .will_subscribe(RpcRequestKind::SubscribeNewPendingTransactions)
            {
                if let Err(e) = self.send_rpc(RpcRequest::new_pending_transactions(&self.rpc_ids)) {
                    tracing::error!(
                        "EVM: failed to send newPendingTransactions subscription request: {}",
                        e
                    );
                }
            } else {
                tracing::info!("EVM: already have newPendingTransactions subscription or request in flight, not sending another");
            }
        }
    }
}

#[derive(Clone, Default)]
struct LogFilter {
    addresses: HashSet<Address>,
    topics: HashSet<B256>,
}

#[derive(Default)]
struct SubscriptionIds {
    _lookup: std::sync::RwLock<std::collections::HashMap<String, SubscriptionKind>>,
    _unsubscribe_lookup:
        std::sync::RwLock<std::collections::HashMap<UnsubscribeKind, HashSet<String>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubscriptionKind {
    NewHeads,
    Logs {
        addresses: HashSet<Address>,
        topics: HashSet<B256>,
    },
    NewPendingTransactions,
}

impl TryFrom<RpcRequestKind> for SubscriptionKind {
    type Error = &'static str;

    fn try_from(value: RpcRequestKind) -> Result<Self, Self::Error> {
        match value {
            RpcRequestKind::SubscribeNewHeads => Ok(SubscriptionKind::NewHeads),
            RpcRequestKind::SubscribeLogs { addresses, topics } => {
                Ok(SubscriptionKind::Logs { addresses, topics })
            }
            RpcRequestKind::SubscribeNewPendingTransactions => {
                Ok(SubscriptionKind::NewPendingTransactions)
            }
            RpcRequestKind::Unsubscribe { .. } => {
                Err("Cannot convert Unsubscribe to SubscriptionKind")
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum UnsubscribeKind {
    NewHeads,
    AllLogs,
    NewPendingTransactions,
}

impl From<&SubscriptionKind> for UnsubscribeKind {
    fn from(kind: &SubscriptionKind) -> Self {
        match kind {
            SubscriptionKind::NewHeads => UnsubscribeKind::NewHeads,
            SubscriptionKind::Logs { .. } => UnsubscribeKind::AllLogs,
            SubscriptionKind::NewPendingTransactions => UnsubscribeKind::NewPendingTransactions,
        }
    }
}

impl TryFrom<&RpcRequestKind> for UnsubscribeKind {
    type Error = &'static str;

    fn try_from(kind: &RpcRequestKind) -> Result<Self, Self::Error> {
        match kind {
            RpcRequestKind::SubscribeNewHeads => Ok(UnsubscribeKind::NewHeads),
            RpcRequestKind::SubscribeLogs { .. } => Ok(UnsubscribeKind::AllLogs),
            RpcRequestKind::SubscribeNewPendingTransactions => {
                Ok(UnsubscribeKind::NewPendingTransactions)
            }
            RpcRequestKind::Unsubscribe { .. } => {
                Err("Cannot convert rpc request for Unsubscribe to UnsubscribeKind")
            }
        }
    }
}

impl SubscriptionIds {
    fn clear(&self) {
        self._lookup.write().unwrap().clear();
        self._unsubscribe_lookup.write().unwrap().clear();
    }

    fn insert(&self, id: String, kind: SubscriptionKind) {
        self._unsubscribe_lookup
            .write()
            .unwrap()
            .entry((&kind).into())
            .or_default()
            .insert(id.clone());

        self._lookup.write().unwrap().insert(id.clone(), kind);
    }

    fn list_by_unsubscribe(&self, kind: UnsubscribeKind) -> Vec<String> {
        match self._unsubscribe_lookup.read().unwrap().get(&kind) {
            Some(ids) => ids.iter().cloned().collect(),
            None => vec![],
        }
    }

    fn remove(&self, id: &str) {
        self._lookup.write().unwrap().remove(id);
        for (_kind, ids) in self._unsubscribe_lookup.write().unwrap().iter_mut() {
            ids.remove(id);
        }
    }

    fn compare(&self, id: &str, f: impl FnOnce(&SubscriptionKind) -> bool) -> bool {
        match self._lookup.read().unwrap().get(id) {
            None => false,
            Some(kind) => f(kind),
        }
    }
}

#[derive(Default)]
struct RpcIdsInFlight {
    _lookup: std::sync::RwLock<std::collections::HashMap<RpcId, RpcFlightState>>,
}

#[derive(Clone, Debug)]
enum RpcFlightState {
    ActivateOnLand {
        // just to avoid unnecessary locks/lookups by calling id.kind() in loops
        kind: RpcRequestKind,
    },
    UnsubscribeOnLand,
}

impl RpcIdsInFlight {
    fn clear(&self) {
        self._lookup.write().unwrap().clear();
    }

    fn insert(&self, id: RpcId, kind: RpcRequestKind) {
        self._lookup
            .write()
            .unwrap()
            .insert(id, RpcFlightState::ActivateOnLand { kind });
    }

    // We want to unsubscribe *all* subscriptions of a given kind, irregardless of log filter
    fn set_unsubscribe(&self, kind: UnsubscribeKind) {
        let lookup = &mut *self._lookup.write().unwrap();

        for (_, state) in lookup.iter_mut() {
            match state {
                RpcFlightState::ActivateOnLand { kind: req_kind }
                    if UnsubscribeKind::try_from(&*req_kind) == Ok(kind) =>
                {
                    *state = RpcFlightState::UnsubscribeOnLand;
                }
                _ => {}
            }
        }
    }

    // unlike setting to unsubscribe, we want to be more specific in checking the kind here,
    // i.e. here we do care about the log filter
    fn will_subscribe(&self, kind: RpcRequestKind) -> bool {
        let lookup = self._lookup.read().unwrap();

        lookup.values().any(|state| match state {
            RpcFlightState::ActivateOnLand { kind: req_kind } => *req_kind == kind,
            _ => false,
        })
    }

    fn remove(&self, id: &RpcId) -> Option<RpcFlightState> {
        self._lookup.write().unwrap().remove(id)
    }

    fn any_active_in_flight(&self) -> bool {
        self._lookup
            .read()
            .unwrap()
            .values()
            .any(|state| matches!(state, RpcFlightState::ActivateOnLand { .. }))
    }
}
