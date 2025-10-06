use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::Log;
use slotmap::Key;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::subsystems::trigger::clients::evm::{
    channels::SubscriptionChannels,
    connection::{ConnectionData, ConnectionState},
    rpc_types::{
        id::{RpcId, RpcRequestKind},
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
    pub fn new(channels: SubscriptionChannels) -> Self {
        let SubscriptionChannels {
            mut subscription_block_height_tx,
            mut subscription_log_tx,
            mut subscription_new_pending_transaction_tx,
            connection_send_rpc_tx,
            mut connection_state_rx,
            mut connection_data_rx,
        } = channels;

        let inner = Arc::new(SubscriptionsInner::new(connection_send_rpc_tx));

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
                                                            inner.on_recieved_subscription_event(
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
                                    RpcId::clear_all();
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
        self.inner.set_blocks(value);
    }

    pub fn enable_log(&self, address: Option<Address>, event: Option<B256>) {
        self.inner.insert_log(address, event);
    }

    pub fn enable_logs(&self, addresses: Vec<Address>, events: Vec<B256>) {
        self.inner.insert_logs(addresses, events);
    }

    pub fn disable_log(&self, address: Option<Address>, event: Option<B256>) {
        self.inner.remove_log(address, event);
    }

    pub fn disable_logs(&self, addresses: Vec<Address>, events: Vec<B256>) {
        self.inner.remove_logs(addresses, events);
    }

    pub fn toggle_pending_transactions(&self, value: bool) {
        self.inner.set_pending_transactions(value);
    }

    pub fn all_rpc_requests_landed(&self) -> bool {
        self.inner.rpc_ids_in_flight.read().unwrap().is_empty()
    }

    pub fn is_connected(&self) -> bool {
        self.inner
            ._is_connected
            .load(std::sync::atomic::Ordering::SeqCst)
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
    _logs: std::sync::RwLock<LogFilter>,
    _pending_transactions: AtomicBool,
    // not really a subscription, but used to track connection state
    _is_connected: AtomicBool,
    ids: SubscriptionIds,
    rpc_ids_in_flight: std::sync::RwLock<HashSet<RpcId>>,
    rpc_ids_to_unsubscribe_on_landing: std::sync::RwLock<HashSet<RpcId>>,
    _connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>,
}

impl SubscriptionsInner {
    pub fn new(connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>) -> Self {
        Self {
            _blocks: AtomicBool::new(false),
            _logs: std::sync::RwLock::new(LogFilter::default()),
            _pending_transactions: AtomicBool::new(false),
            _is_connected: AtomicBool::new(false),
            ids: SubscriptionIds::default(),
            rpc_ids_in_flight: std::sync::RwLock::new(HashSet::new()),
            rpc_ids_to_unsubscribe_on_landing: std::sync::RwLock::new(HashSet::new()),
            _connection_send_rpc_tx: connection_send_rpc_tx,
        }
    }

    pub fn set_blocks(&self, value: bool) {
        self._blocks
            .store(value, std::sync::atomic::Ordering::SeqCst);

        if !value {
            // no need to resubscribe in this case
            // logs is different since changing the filter requires a resubscribe
            self.unsubscribe(SubscriptionKind::NewHeads);
        } else {
            self.resubscribe_if_connected();
        }
    }

    pub fn insert_log(&self, addresses: Option<Address>, events: Option<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            if let Some(address) = addresses {
                lock.addresses.insert(address);
            }
            if let Some(event) = events {
                lock.events.insert(event);
            }
        }
        self.unsubscribe(SubscriptionKind::Logs);
        self.resubscribe_if_connected();
    }

    pub fn remove_log(&self, addresses: Option<Address>, events: Option<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            if let Some(address) = addresses {
                lock.addresses.remove(&address);
            }
            if let Some(event) = events {
                lock.events.remove(&event);
            }
        }
        self.unsubscribe(SubscriptionKind::Logs);
        self.resubscribe_if_connected();
    }

    pub fn insert_logs(&self, address: Vec<Address>, event: Vec<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            for address in address {
                lock.addresses.insert(address);
            }

            for event in event {
                lock.events.insert(event);
            }
        }
        self.unsubscribe(SubscriptionKind::Logs);
        self.resubscribe_if_connected();
    }

    pub fn remove_logs(&self, addresses: Vec<Address>, events: Vec<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            for address in addresses {
                lock.addresses.remove(&address);
            }

            for event in events {
                lock.events.remove(&event);
            }
        }
        self.unsubscribe(SubscriptionKind::Logs);
        self.resubscribe_if_connected();
    }

    pub fn set_pending_transactions(&self, value: bool) {
        self._pending_transactions
            .store(value, std::sync::atomic::Ordering::SeqCst);
        if !value {
            // no need to resubscribe in this case
            // logs is different since changing the filter requires a resubscribe
            self.unsubscribe(SubscriptionKind::NewPendingTransactions);
        } else {
            self.resubscribe_if_connected();
        }
    }

    pub fn set_is_connected(&self, value: bool) {
        self._is_connected
            .store(value, std::sync::atomic::Ordering::SeqCst);

        if !value {
            self.ids.clear();
        } else {
            self.resubscribe_if_connected();
        }
    }

    // all requests must go through here so we can track in-flight requests
    fn send_rpc(
        &self,
        req: RpcRequest,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<RpcRequest>> {
        self.rpc_ids_in_flight.write().unwrap().insert(req.id());

        self._connection_send_rpc_tx.send(req)
    }

    fn on_received_rpc_response(&self, id: RpcId, response: RpcResponse) {
        self.rpc_ids_in_flight.write().unwrap().remove(&id);
        // since we clear the rpc ids (and sub ids) on disconnect,
        // we can be sure that any new subscription id we get is for this connection
        let kind = match id.kind() {
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
                if self
                    .rpc_ids_to_unsubscribe_on_landing
                    .write()
                    .unwrap()
                    .remove(&id)
                {
                    if let Err(e) = self.send_rpc(RpcRequest::unsubscribe(subscription_id.clone()))
                    {
                        tracing::error!(
                            "EVM: failed to send unsubscribe request for subscription id {}: {}",
                            subscription_id,
                            e
                        );
                    } else {
                        tracing::info!(
                            "EVM: sent unsubscribe request for subscription id {}",
                            subscription_id
                        );
                    }
                    return; // we are done here, don't track this subscription
                }
                match kind {
                    RpcRequestKind::SubscribeNewHeads => {
                        tracing::info!(
                            "EVM: subscribed to newHeads with subscription id {}",
                            subscription_id
                        );
                        self.ids
                            .insert(subscription_id, SubscriptionKind::NewHeads, None);
                    }
                    RpcRequestKind::SubscribeLogs { address, topics } => {
                        tracing::info!(
                            "EVM: subscribed to logs with subscription id {}",
                            subscription_id
                        );
                        self.ids.insert(
                            subscription_id,
                            SubscriptionKind::Logs,
                            Some((address, topics)),
                        );
                    }
                    RpcRequestKind::SubscribeNewPendingTransactions => {
                        tracing::info!(
                            "EVM: subscribed to newPendingTransactions with subscription id {}",
                            subscription_id
                        );
                        self.ids.insert(
                            subscription_id,
                            SubscriptionKind::NewPendingTransactions,
                            None,
                        );
                    }
                    RpcRequestKind::Unsubscribe { subscription_id } => {
                        tracing::error!("EVM: received newSubscription response for unsubscribe request id {} (subscription id: {})", id.data().as_ffi(), subscription_id);
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

    fn on_recieved_subscription_event(
        &self,
        subscription_block_height_tx: &mut tokio::sync::mpsc::UnboundedSender<u64>,
        subscription_log_tx: &mut tokio::sync::mpsc::UnboundedSender<Log>,
        subscription_new_pending_transaction_tx: &mut tokio::sync::mpsc::UnboundedSender<B256>,
        subscription_id: String,
        event: RpcSubscriptionEvent,
    ) {
        match event {
            RpcSubscriptionEvent::NewHeads(header) => {
                if !self.ids.eq(&subscription_id, SubscriptionKind::NewHeads) {
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

                if !self.ids.eq(&subscription_id, SubscriptionKind::Logs) {
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
                if !self
                    .ids
                    .eq(&subscription_id, SubscriptionKind::NewPendingTransactions)
                {
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

    fn unsubscribe(&self, kind: SubscriptionKind) {
        let ids = self.ids.list(kind);

        {
            let mut rpcs_to_unsubscribe_on_landing =
                self.rpc_ids_to_unsubscribe_on_landing.write().unwrap();
            let rpc_ids_in_flight = self.rpc_ids_in_flight.read().unwrap();

            for rpc_id in rpc_ids_in_flight.iter() {
                let to_unsubscribe = matches!(
                    (kind, rpc_id.kind()),
                    (
                        SubscriptionKind::NewHeads,
                        Some(RpcRequestKind::SubscribeNewHeads)
                    ) | (
                        SubscriptionKind::Logs,
                        Some(RpcRequestKind::SubscribeLogs { .. })
                    ) | (
                        SubscriptionKind::NewPendingTransactions,
                        Some(RpcRequestKind::SubscribeNewPendingTransactions),
                    )
                );

                if to_unsubscribe {
                    rpcs_to_unsubscribe_on_landing.insert(*rpc_id);

                    tracing::info!(
                        "EVM: Marked RPC id to unsubscribe on landing: {}",
                        rpc_id.data().as_ffi()
                    );
                }
            }
        }

        for id in ids {
            if let Err(e) = self.send_rpc(RpcRequest::unsubscribe(id.clone())) {
                tracing::error!(
                    "EVM: failed to send unsubscribe request for subscription id {}: {}",
                    id,
                    e
                );
            } else {
                tracing::info!("EVM: sent unsubscribe request for subscription id {}", id);
            }
            self.ids.remove(&id);
        }
    }

    fn will_subscribe(&self, kind: RpcRequestKind) -> bool {
        let rpc_ids_in_flight = self.rpc_ids_in_flight.read().unwrap();
        let rpc_ids_marked_for_removal = self.rpc_ids_to_unsubscribe_on_landing.read().unwrap();

        rpc_ids_in_flight.iter().any(|id| {
            // okay, it's in flight and will subscribe if it lands... maybe...
            if id.kind() == Some(kind.clone()) {
                // because if it's marked for removal then it will *not* subscribe when it lands
                !rpc_ids_marked_for_removal.contains(id)
            } else {
                false
            }
        })
    }

    fn resubscribe_if_connected(&self) {
        // exit early if not connected
        if !self._is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        // blocks/newHeads
        if self._blocks.load(std::sync::atomic::Ordering::SeqCst) {
            if !self.ids.any(SubscriptionKind::NewHeads)
                && !self.will_subscribe(RpcRequestKind::SubscribeNewHeads)
            {
                let req = RpcRequest::new_heads();
                let req_id = req.id();
                if let Err(e) = self.send_rpc(req) {
                    tracing::error!("EVM: failed to send newHeads subscription request: {}", e);
                } else {
                    tracing::info!(
                        "EVM: sent newHeads subscription request (id: {})",
                        req_id.data().as_ffi()
                    );
                }
            } else {
                tracing::info!("EVM: already have newHeads subscription or request in flight, not sending another");
            }
        }

        // logs
        {
            // logs is a bit tricky, the test is against the specific log filter, not just the high-level kind
            // because we can have multiple different log filters active at once while they are still unsubscribed
            let (addresses, events) = {
                let lock = self._logs.read().unwrap();
                (lock.addresses.clone(), lock.events.clone())
            };

            if !addresses.is_empty() || !events.is_empty() {
                if !self.ids.any_log_filter(&addresses, &events)
                    && !self.will_subscribe(RpcRequestKind::SubscribeLogs {
                        address: addresses.clone(),
                        topics: events.clone(),
                    })
                {
                    let req = RpcRequest::logs(addresses, events);
                    let req_id = req.id();
                    if let Err(e) = self.send_rpc(req) {
                        tracing::error!("EVM: failed to send logs subscription request: {}", e);
                    } else {
                        tracing::info!(
                            "EVM: sent logs subscription request (id: {})",
                            req_id.data().as_ffi()
                        );
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
            if !self.ids.any(SubscriptionKind::NewPendingTransactions)
                && !self.will_subscribe(RpcRequestKind::SubscribeNewPendingTransactions)
            {
                let req = RpcRequest::new_pending_transactions();
                let req_id = req.id();
                if let Err(e) = self.send_rpc(req) {
                    tracing::error!(
                        "EVM: failed to send newPendingTransactions subscription request: {}",
                        e
                    );
                } else {
                    tracing::info!(
                        "EVM: sent newPendingTransactions subscription request (id: {})",
                        req_id.data().as_ffi()
                    );
                }
            } else {
                tracing::info!("EVM: already have newPendingTransactions subscription or request in flight, not sending another");
            }
        }
    }
}

#[derive(Default)]
struct LogFilter {
    addresses: HashSet<Address>,
    events: HashSet<B256>,
}

#[derive(Default)]
struct SubscriptionIds {
    _lookup: std::sync::RwLock<std::collections::HashMap<String, SubscriptionKind>>,
    _reverse_lookup:
        std::sync::RwLock<std::collections::HashMap<SubscriptionKind, HashSet<String>>>,
    _log_filters: std::sync::RwLock<HashMap<String, (HashSet<Address>, HashSet<B256>)>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum SubscriptionKind {
    NewHeads,
    Logs,
    NewPendingTransactions,
}

impl SubscriptionIds {
    fn clear(&self) {
        self._lookup.write().unwrap().clear();
        self._reverse_lookup.write().unwrap().clear();
        self._log_filters.write().unwrap().clear();
    }

    fn any(&self, kind: SubscriptionKind) -> bool {
        match self._reverse_lookup.read().unwrap().get(&kind) {
            Some(ids) => !ids.is_empty(),
            None => false,
        }
    }

    fn insert(
        &self,
        id: String,
        kind: SubscriptionKind,
        log_filters: Option<(HashSet<Address>, HashSet<B256>)>,
    ) {
        self._lookup.write().unwrap().insert(id.clone(), kind);
        self._reverse_lookup
            .write()
            .unwrap()
            .entry(kind)
            .or_default()
            .insert(id.clone());

        if let Some((addresses, events)) = log_filters {
            self._log_filters.write().unwrap().insert(
                id,
                (
                    addresses.into_iter().collect(),
                    events.into_iter().collect(),
                ),
            );
        }
    }

    fn any_log_filter(&self, addresses: &HashSet<Address>, events: &HashSet<B256>) -> bool {
        let lock = self._log_filters.read().unwrap();
        for (addr_set, event_set) in lock.values() {
            if addr_set == addresses && event_set == events {
                return true;
            }
        }
        false
    }

    fn list(&self, kind: SubscriptionKind) -> Vec<String> {
        match self._reverse_lookup.read().unwrap().get(&kind) {
            Some(ids) => ids.iter().cloned().collect(),
            None => vec![],
        }
    }

    fn remove(&self, id: &str) {
        self._lookup.write().unwrap().remove(id);
        for (_kind, ids) in self._reverse_lookup.write().unwrap().iter_mut() {
            ids.remove(id);
        }
        self._log_filters.write().unwrap().remove(id);
    }

    fn eq(&self, id: &str, kind: SubscriptionKind) -> bool {
        matches!(self._lookup.read().unwrap().get(id),  Some(current_kind) if *current_kind == kind)
    }
}
