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

const UNSUBSCRIBE_RETRY_SECS: u64 = 5;

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
            self.unsubscribe(SubscriptionCategory::NewHeads);
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
        self.unsubscribe(SubscriptionCategory::AllLogs);

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
        self.unsubscribe(SubscriptionCategory::AllLogs);
        self.resubscribe();
    }

    pub fn disable_all_logs(&self) {
        *self._logs.write().unwrap() = None;
        self.unsubscribe(SubscriptionCategory::AllLogs);
        // no need to resubscribe
    }

    pub fn toggle_pending_transactions(&self, toggle: bool) {
        self._pending_transactions
            .store(toggle, std::sync::atomic::Ordering::SeqCst);
        if !toggle {
            self.unsubscribe(SubscriptionCategory::NewPendingTransactions);
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
        delay: Option<Duration>,
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
            match delay {
                Some(d) => {
                    let tx = self._connection_send_rpc_tx.clone();
                    let rpc_ids_in_flight = self.rpc_ids_in_flight.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(d).await;

                        // connection might have been reset during the delay
                        // so only send if it's still considered in-flight
                        let should_send = match rpc_ids_in_flight.get(&req.id()) {
                            Some(RpcFlightState::ActivateOnLand { kind }) => {
                                match kind {
                                    RpcRequestKind::Unsubscribe {
                                        subscription_id: sub_id_1,
                                    } => match &req {
                                        RpcRequest::Unsubscribe {
                                            subscription_id: sub_id_2,
                                            ..
                                        } => sub_id_1 == *sub_id_2,
                                        _ => {
                                            tracing::warn!("Unexpected mismatch when checking for delayed unsubscribe");
                                            false
                                        }
                                    },
                                    _ => true, // for now, always send other requests too
                                }
                            }
                            _ => false,
                        };

                        if should_send {
                            if let Err(e) = tx.send(req) {
                                tracing::error!("EVM: failed to send delayed RPC request: {}", e);
                            }
                        }
                    });
                }
                None => {
                    self._connection_send_rpc_tx.send(req)?;
                }
            }
        } else {
            tracing::warn!(
                "couldn't get in-flight kind for rpc id {}",
                req.id().data().as_ffi()
            );
        }

        Ok(())
    }

    fn on_received_rpc_response(&self, id: RpcId, response: RpcResponse) {
        let (removed_rpc_in_flight_state, removed_rpc_latest_subscription_category) =
            self.rpc_ids_in_flight.remove(&id);
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
                    if let Err(e) = self.send_rpc(
                        RpcRequest::unsubscribe(&self.rpc_ids, subscription_id.clone()),
                        None,
                    ) {
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
                        let most_recent_category = {
                            match removed_rpc_latest_subscription_category {
                                Some(category) => {
                                    let cat = SubscriptionCategory::from(&kind);
                                    // just a sanity check
                                    if cat != category {
                                        tracing::warn!("EVM: weird, got mismatched subscription category for rpc id {}: expected {:?}, got {:?}", id.data().as_ffi(), cat, category);
                                    }
                                    true
                                }
                                None => false,
                            }
                        };

                        // theoretically we should only ever insert when most_recent_category is true
                        // but then we would miss events if the less-recent subscription landed first
                        // and an event fires before the most-recent one lands
                        self.ids.insert(subscription_id, kind, most_recent_category);
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
                            if self.ids.exists(&subscription_id) {
                                tracing::warn!(
                                    "EVM: failed to unsubscribe from subscription id {}, trying again in {} seconds",
                                    subscription_id,
                                    UNSUBSCRIBE_RETRY_SECS
                                );
                                if let Err(e) = self.send_rpc(
                                    RpcRequest::unsubscribe(&self.rpc_ids, subscription_id.clone()),
                                    Some(Duration::from_secs(UNSUBSCRIBE_RETRY_SECS)),
                                ) {
                                    tracing::error!(
                                        "EVM: failed to send re-unsubscribe request for subscription id {}: {}",
                                        subscription_id,
                                        e
                                    );
                                }
                            }
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
                } else if self
                    .ids
                    .is_most_recent(&subscription_id, SubscriptionCategory::NewHeads)
                {
                    if let Err(e) = subscription_block_height_tx.send(header.number) {
                        tracing::error!("EVM: failed to send new block height: {}", e);
                    }
                } else {
                    tracing::info!("EVM: ignoring newHeads event for non-most-recent newHeads subscription id {}", subscription_id);
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
                } else if self
                    .ids
                    .is_most_recent(&subscription_id, SubscriptionCategory::AllLogs)
                {
                    if let Err(e) = subscription_log_tx.send(log) {
                        tracing::error!("EVM: failed to send log: {}", e);
                    }
                } else {
                    tracing::info!(
                        "EVM: ignoring log event for non-most-recent logs subscription id {}",
                        subscription_id
                    );
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
                } else if self.ids.is_most_recent(
                    &subscription_id,
                    SubscriptionCategory::NewPendingTransactions,
                ) {
                    if let Err(e) = subscription_new_pending_transaction_tx.send(tx) {
                        tracing::error!("EVM: failed to send new pending transaction: {}", e);
                    }
                } else {
                    tracing::info!("EVM: ignoring new pending transaction event for non-most-recent newPendingTransactions subscription id {}", subscription_id);
                }
            }
        }
    }

    fn unsubscribe(&self, category: SubscriptionCategory) {
        let ids = self.ids.list_by_category(category);

        // mark the rpcs in flight to unsubscribe when they land
        self.rpc_ids_in_flight.set_unsubscribe(category);

        // send the unsubscribe request for the active subscriptions
        // will actually unsubscribe when the ack response lands
        for id in ids {
            if let Err(e) = self.send_rpc(RpcRequest::unsubscribe(&self.rpc_ids, id.clone()), None)
            {
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
                if let Err(e) = self.send_rpc(RpcRequest::new_heads(&self.rpc_ids), None) {
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
                        self.send_rpc(RpcRequest::logs(&self.rpc_ids, addresses, topics), None)
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
                if let Err(e) =
                    self.send_rpc(RpcRequest::new_pending_transactions(&self.rpc_ids), None)
                {
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
    _categories: std::sync::RwLock<
        std::collections::HashMap<SubscriptionCategory, SubscriptionIdCategories>,
    >,
}

#[derive(Default)]
struct SubscriptionIdCategories {
    ids: HashSet<String>,
    most_recent: Option<String>,
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

// this is used to track:
// * unsubscriptions, since we want to unsubscribe *all* subscriptions of a given kind (e.g. ignore the log filter)
// * latest-and-greatest subscriptions, since we only ever want one active subscription of a given kind (e.g. newHeads, newPendingTransactions, logs of *any* filter)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum SubscriptionCategory {
    NewHeads,
    AllLogs,
    NewPendingTransactions,
}

impl From<&SubscriptionKind> for SubscriptionCategory {
    fn from(kind: &SubscriptionKind) -> Self {
        match kind {
            SubscriptionKind::NewHeads => SubscriptionCategory::NewHeads,
            SubscriptionKind::Logs { .. } => SubscriptionCategory::AllLogs,
            SubscriptionKind::NewPendingTransactions => {
                SubscriptionCategory::NewPendingTransactions
            }
        }
    }
}

impl TryFrom<&RpcRequestKind> for SubscriptionCategory {
    type Error = &'static str;

    fn try_from(kind: &RpcRequestKind) -> Result<Self, Self::Error> {
        match kind {
            RpcRequestKind::SubscribeNewHeads => Ok(SubscriptionCategory::NewHeads),
            RpcRequestKind::SubscribeLogs { .. } => Ok(SubscriptionCategory::AllLogs),
            RpcRequestKind::SubscribeNewPendingTransactions => {
                Ok(SubscriptionCategory::NewPendingTransactions)
            }
            RpcRequestKind::Unsubscribe { .. } => {
                Err("Cannot convert rpc request for Unsubscribe to SubscriptionCategory")
            }
        }
    }
}

impl SubscriptionIds {
    fn clear(&self) {
        self._lookup.write().unwrap().clear();
        self._categories.write().unwrap().clear();
    }

    fn insert(&self, id: String, kind: SubscriptionKind, most_recent_category: bool) {
        let mut categories = self._categories.write().unwrap();
        let categories = categories.entry((&kind).into()).or_default();

        categories.ids.insert(id.clone());

        if most_recent_category {
            categories.most_recent = Some(id.clone());
        }

        self._lookup.write().unwrap().insert(id, kind);
    }

    fn list_by_category(&self, category: SubscriptionCategory) -> Vec<String> {
        match self._categories.read().unwrap().get(&category) {
            Some(x) => x.ids.iter().cloned().collect(),
            None => vec![],
        }
    }

    fn is_most_recent(&self, id: &str, category: SubscriptionCategory) -> bool {
        match self._categories.read().unwrap().get(&category) {
            Some(x) => match &x.most_recent {
                Some(most_recent_id) => most_recent_id == id,
                // if we don't have any most-recent id, treat all as most recent
                // i.e. until one of them lands
                None => true,
            },
            None => false,
        }
    }

    fn remove(&self, id: &str) {
        self._lookup.write().unwrap().remove(id);
        for (_kind, category) in self._categories.write().unwrap().iter_mut() {
            category.ids.remove(id);
            if category.most_recent.as_deref() == Some(id) {
                category.most_recent = None;
            }
        }
    }

    fn compare(&self, id: &str, f: impl FnOnce(&SubscriptionKind) -> bool) -> bool {
        match self._lookup.read().unwrap().get(id) {
            None => false,
            Some(kind) => f(kind),
        }
    }

    fn exists(&self, id: &str) -> bool {
        self._lookup.read().unwrap().contains_key(id)
    }
}

#[derive(Clone, Default)]
struct RpcIdsInFlight {
    _lookup: Arc<std::sync::RwLock<std::collections::HashMap<RpcId, RpcFlightState>>>,
    _most_recent_subscription_category:
        Arc<std::sync::RwLock<HashMap<SubscriptionCategory, RpcId>>>,
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
        self._most_recent_subscription_category
            .write()
            .unwrap()
            .clear();
    }

    fn insert(&self, id: RpcId, kind: RpcRequestKind) {
        if let Ok(category) = SubscriptionCategory::try_from(&kind) {
            self._most_recent_subscription_category
                .write()
                .unwrap()
                .insert(category, id);
        }
        self._lookup
            .write()
            .unwrap()
            .insert(id, RpcFlightState::ActivateOnLand { kind });
    }

    // We want to unsubscribe *all* subscriptions of a given kind, irregardless of log filter
    fn set_unsubscribe(&self, category: SubscriptionCategory) {
        let lookup = &mut *self._lookup.write().unwrap();

        for (_, state) in lookup.iter_mut() {
            match state {
                RpcFlightState::ActivateOnLand { kind: req_kind }
                    if SubscriptionCategory::try_from(&*req_kind) == Ok(category) =>
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

    // returns and removes the state if it existed
    // also removes and returns the subscription category if this rpc_id was the most recent of its category
    fn remove(&self, id: &RpcId) -> (Option<RpcFlightState>, Option<SubscriptionCategory>) {
        let removed_state = self._lookup.write().unwrap().remove(id);

        let category = removed_state.as_ref().and_then(|state| {
            match state {
                RpcFlightState::ActivateOnLand { kind } => {
                    SubscriptionCategory::try_from(kind).ok()
                }
                RpcFlightState::UnsubscribeOnLand => None, // we don't care about unsubscribes here
            }
        });

        let removed_category = match category {
            Some(cat) => {
                let mut categories = self._most_recent_subscription_category.write().unwrap();
                match categories.get(&cat) {
                    Some(most_recent_id) if most_recent_id == id => {
                        categories.remove(&cat);
                        Some(cat)
                    }
                    _ => None,
                }
            }
            None => None,
        };

        (removed_state, removed_category)
    }

    fn get(&self, id: &RpcId) -> Option<RpcFlightState> {
        self._lookup.read().unwrap().get(id).cloned()
    }

    fn any_active_in_flight(&self) -> bool {
        self._lookup
            .read()
            .unwrap()
            .values()
            .any(|state| matches!(state, RpcFlightState::ActivateOnLand { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, b256, Bytes, B256};
    use alloy_rpc_types_eth::Header;
    use serde_json::json;
    use std::collections::HashSet;
    use tokio::sync::mpsc::unbounded_channel;

    fn new_inner() -> (
        SubscriptionsInner,
        tokio::sync::mpsc::UnboundedReceiver<RpcRequest>,
    ) {
        let rpc_ids = RpcIds::new();
        let (connection_tx, connection_rx) = unbounded_channel();
        // Return the inner subscriptions object alongside the channel we use to inspect outbound RPCs.
        (
            SubscriptionsInner::new(rpc_ids, connection_tx),
            connection_rx,
        )
    }

    fn sample_log(address: Address, topic: B256) -> alloy_rpc_types_eth::Log {
        alloy_rpc_types_eth::Log::<alloy_primitives::LogData> {
            inner: alloy_primitives::Log::new_unchecked(address, vec![topic], Bytes::new()),
            ..Default::default()
        }
    }

    fn sample_header(number: u64) -> Header {
        // Build an RPC header without depending on extra crates so tests can emit block events.
        let value = json!({
            "hash": format!("0x{:064x}", number + 1),
            "parentHash": format!("0x{:064x}", number + 2),
            "sha3Uncles": format!("0x{:064x}", number + 3),
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": format!("0x{:064x}", number + 4),
            "transactionsRoot": format!("0x{:064x}", number + 5),
            "receiptsRoot": format!("0x{:064x}", number + 6),
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "difficulty": "0x1",
            "number": format!("0x{:x}", number),
            "gasLimit": "0x1",
            "gasUsed": "0x1",
            "timestamp": "0x1",
            "extraData": "0x",
            "mixHash": format!("0x{:064x}", number + 7),
            "nonce": "0x0000000000000000"
        });

        serde_json::from_value(value).expect("valid header JSON")
    }

    #[tokio::test]
    async fn pending_transaction_events_follow_most_recent_subscription() {
        let (inner, mut connection_rx) = new_inner();

        // Prepare channels used by on_received_subscription_event.
        let (block_tx, _block_rx) = unbounded_channel();
        let (log_tx, _log_rx) = unbounded_channel();
        let (pending_tx, mut pending_rx) = unbounded_channel();
        let mut block_tx = block_tx;
        let mut log_tx = log_tx;
        let mut pending_tx = pending_tx;

        // Send three pending-transaction subscription requests in sequence so the
        // final request is the most recent one tracked.
        let req1 = RpcRequest::new_pending_transactions(&inner.rpc_ids);
        let id1 = req1.id();
        inner.send_rpc(req1, None).unwrap();

        let req2 = RpcRequest::new_pending_transactions(&inner.rpc_ids);
        let id2 = req2.id();
        inner.send_rpc(req2, None).unwrap();

        let req3 = RpcRequest::new_pending_transactions(&inner.rpc_ids);
        let id3 = req3.id();
        inner.send_rpc(req3, None).unwrap();

        // Drain the outbound requests – not strictly required for state-tracking,
        // but ensures the channel stays empty for the remainder of the test.
        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));
        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));
        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));

        // Responses arrive out of order: the two older requests land before the most
        // recent one. Until the most recent subscription completes we expect events
        // from the older subscriptions to be forwarded.
        inner.on_received_rpc_response(
            id1,
            RpcResponse::NewSubscription {
                subscription_id: "sub-1".into(),
            },
        );
        inner.on_received_rpc_response(
            id2,
            RpcResponse::NewSubscription {
                subscription_id: "sub-2".into(),
            },
        );

        let first_hash = B256::from([1u8; 32]);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-1".into(),
            RpcSubscriptionEvent::NewPendingTransaction(first_hash),
        );
        assert_eq!(pending_rx.recv().await.unwrap(), first_hash);

        let second_hash = B256::from([4u8; 32]);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-2".into(),
            RpcSubscriptionEvent::NewPendingTransaction(second_hash),
        );
        assert_eq!(pending_rx.recv().await.unwrap(), second_hash);

        // The newest subscription finishes last and becomes the active stream.
        inner.on_received_rpc_response(
            id3,
            RpcResponse::NewSubscription {
                subscription_id: "sub-3".into(),
            },
        );
        assert!(inner
            .ids
            .is_most_recent("sub-3", SubscriptionCategory::NewPendingTransactions));

        // Once sub-3 has landed, prior subscriptions should be treated as stale.
        // Events for older subscriptions are now ignored.
        let ignored_hash = B256::from([2u8; 32]);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-1".into(),
            RpcSubscriptionEvent::NewPendingTransaction(ignored_hash),
        );
        assert!(pending_rx.try_recv().is_err());

        // Events for the active subscription still flow through.
        let latest_hash = B256::from([3u8; 32]);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-3".into(),
            RpcSubscriptionEvent::NewPendingTransaction(latest_hash),
        );
        assert_eq!(pending_rx.recv().await.unwrap(), latest_hash);
    }

    #[tokio::test]
    async fn log_events_follow_most_recent_subscription_per_filter() {
        let (inner, mut connection_rx) = new_inner();

        let (block_tx, _block_rx) = unbounded_channel();
        let (log_tx, mut log_rx) = unbounded_channel();
        let (pending_tx, _pending_rx) = unbounded_channel();
        let mut block_tx = block_tx;
        let mut log_tx = log_tx;
        let mut pending_tx = pending_tx;

        // First filter we subscribe to — represents the “stale” stream once we switch over.
        let addresses_first: HashSet<_> = [address!("0x0000000000000000000000000000000000000001")]
            .into_iter()
            .collect();
        let topics_first: HashSet<_> = [b256!(
            "0x0000000000000000000000000000000000000000000000000000000000000001"
        )]
        .into_iter()
        .collect();
        // Second filter is considered most recent and should ultimately win.
        let addresses_second: HashSet<_> = [address!("0x0000000000000000000000000000000000000002")]
            .into_iter()
            .collect();
        let topics_second: HashSet<_> = [b256!(
            "0x0000000000000000000000000000000000000000000000000000000000000002"
        )]
        .into_iter()
        .collect();

        let req1 = RpcRequest::logs(
            &inner.rpc_ids,
            addresses_first.clone(),
            topics_first.clone(),
        );
        let id1 = req1.id();
        inner.send_rpc(req1, None).unwrap();

        let req2 = RpcRequest::logs(
            &inner.rpc_ids,
            addresses_second.clone(),
            topics_second.clone(),
        );
        let id2 = req2.id();
        inner.send_rpc(req2, None).unwrap();

        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));
        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));

        let first_log = sample_log(
            *addresses_first.iter().next().unwrap(),
            *topics_first.iter().next().unwrap(),
        );
        inner.on_received_rpc_response(
            id1,
            RpcResponse::NewSubscription {
                subscription_id: "sub-log-1".into(),
            },
        );
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-log-1".into(),
            RpcSubscriptionEvent::Logs(first_log.clone()),
        );
        assert_eq!(log_rx.recv().await.unwrap(), first_log);

        inner.on_received_rpc_response(
            id2,
            RpcResponse::NewSubscription {
                subscription_id: "sub-log-2".into(),
            },
        );

        let second_log = sample_log(
            *addresses_second.iter().next().unwrap(),
            *topics_second.iter().next().unwrap(),
        );
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-log-2".into(),
            RpcSubscriptionEvent::Logs(second_log.clone()),
        );
        assert_eq!(log_rx.recv().await.unwrap(), second_log);
        assert!(log_rx.try_recv().is_err());
        assert!(inner
            .ids
            .is_most_recent("sub-log-2", SubscriptionCategory::AllLogs));

        // Events from the older subscription are ignored once a different filter becomes active.
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-log-1".into(),
            RpcSubscriptionEvent::Logs(first_log.clone()),
        );
        assert!(log_rx.try_recv().is_err());

        // Active subscription continues to flow events so downstream receivers keep working.
        let third_log = sample_log(
            *addresses_second.iter().next().unwrap(),
            *topics_second.iter().next().unwrap(),
        );
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-log-2".into(),
            RpcSubscriptionEvent::Logs(third_log.clone()),
        );
        assert_eq!(log_rx.recv().await.unwrap(), third_log);
        assert!(log_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn new_head_events_follow_most_recent_subscription() {
        let (inner, mut connection_rx) = new_inner();

        let (block_tx, mut block_rx) = unbounded_channel();
        let (log_tx, _log_rx) = unbounded_channel();
        let (pending_tx, _pending_rx) = unbounded_channel();
        let mut block_tx = block_tx;
        let mut log_tx = log_tx;
        let mut pending_tx = pending_tx;

        // Issue two newHeads subscriptions so we can swap from the first to the second.
        let req1 = RpcRequest::new_heads(&inner.rpc_ids);
        let id1 = req1.id();
        inner.send_rpc(req1, None).unwrap();

        let req2 = RpcRequest::new_heads(&inner.rpc_ids);
        let id2 = req2.id();
        inner.send_rpc(req2, None).unwrap();

        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));
        assert!(matches!(
            connection_rx.recv().await,
            Some(RpcRequest::Subscribe { .. })
        ));

        // The first subscription lands and immediately emits a height update.
        inner.on_received_rpc_response(
            id1,
            RpcResponse::NewSubscription {
                subscription_id: "sub-head-1".into(),
            },
        );
        let header1 = sample_header(111);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-head-1".into(),
            RpcSubscriptionEvent::NewHeads(header1.clone()),
        );
        assert_eq!(block_rx.recv().await.unwrap(), 111);

        // Now the second (most recent) subscription lands and should supersede the first.
        inner.on_received_rpc_response(
            id2,
            RpcResponse::NewSubscription {
                subscription_id: "sub-head-2".into(),
            },
        );
        let header2 = sample_header(222);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-head-2".into(),
            RpcSubscriptionEvent::NewHeads(header2.clone()),
        );
        assert_eq!(block_rx.recv().await.unwrap(), 222);

        // The first subscription should no longer deliver events.
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-head-1".into(),
            RpcSubscriptionEvent::NewHeads(header1),
        );
        assert!(block_rx.try_recv().is_err());

        // Most recent subscription still delivers events, proving we only dropped the stale ones.
        let header3 = sample_header(333);
        inner.on_received_subscription_event(
            &mut block_tx,
            &mut log_tx,
            &mut pending_tx,
            "sub-head-2".into(),
            RpcSubscriptionEvent::NewHeads(header3.clone()),
        );
        assert_eq!(block_rx.recv().await.unwrap(), 333);
        assert!(block_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn unsubscribe_is_sent_when_marked_to_land() {
        let (inner, mut connection_rx) = new_inner();

        let req = RpcRequest::new_heads(&inner.rpc_ids);
        let id = req.id();
        inner.send_rpc(req, None).unwrap();

        let first_outbound = connection_rx.recv().await.unwrap();
        assert!(matches!(first_outbound, RpcRequest::Subscribe { .. }));

        inner
            .rpc_ids_in_flight
            .set_unsubscribe(SubscriptionCategory::NewHeads);

        // When the subscribe response lands the in-flight tracker should immediately emit unsubscribe.
        inner.on_received_rpc_response(
            id,
            RpcResponse::NewSubscription {
                subscription_id: "sub-head-ephemeral".into(),
            },
        );

        match connection_rx.recv().await {
            Some(RpcRequest::Unsubscribe {
                subscription_id, ..
            }) => assert_eq!(subscription_id, "sub-head-ephemeral"),
            other => panic!("expected unsubscribe request, got {:?}", other),
        }

        // The subscription id should never be registered because it was queued for removal.
        assert!(!inner.ids.exists("sub-head-ephemeral"));
    }
}
