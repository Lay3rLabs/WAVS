use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use alloy_primitives::{Address, B256};
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

#[derive(Clone)]
pub struct Subscriptions {
    handle: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
    shutdown_tx: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
    active: Arc<ActiveSubscriptions>,
}

impl Subscriptions {
    pub fn new(channels: SubscriptionChannels) -> Self {
        let SubscriptionChannels {
            subscription_block_height_tx,
            subscription_log_tx,
            subscription_new_pending_transaction_tx,
            connection_send_rpc_tx,
            mut connection_state_rx,
            mut connection_data_rx,
        } = channels;

        let active = Arc::new(ActiveSubscriptions::new(connection_send_rpc_tx));

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let handle = tokio::spawn({
            let active = active.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => {
                            tracing::info!("EVM: shutdown requested, exiting subscription loop");
                            break;
                        }

                        Some(msg) = connection_data_rx.recv() => {
                            enum LocalResult {
                                Rpc {
                                    id: RpcId,
                                    response: RpcResponse
                                },
                                Subscription {
                                    id: String,
                                    event: RpcSubscriptionEvent
                                }
                            }
                            let result = match msg {
                                ConnectionData::Text(text) => {
                                    match serde_json::from_str::<RpcInbound>(&text) {
                                        Ok(inbound) => {
                                            match inbound {
                                                RpcInbound::Response {id, result} => {
                                                    match result {
                                                        Ok(response) => {
                                                            LocalResult::Rpc { id, response }
                                                        }
                                                        Err(err) => {
                                                            tracing::error!("EVM: RPC error for id {}: {:?}", id.data().as_ffi(), err);
                                                            continue;
                                                        }
                                                    }
                                                },
                                                RpcInbound::Subscription{id, result } => {
                                                    match result {
                                                        Ok(event) => {
                                                            LocalResult::Subscription { id, event }
                                                        }
                                                        Err(err) => {
                                                            tracing::error!("EVM: Subscription error for id {}: {:?}", id, err);
                                                            continue;
                                                        }
                                                    }
                                                },

                                            }
                                        },
                                        Err(e) => {
                                            tracing::error!("EVM: failed to parse RPC response: {}", e);
                                            continue;
                                        }
                                    }
                                },
                                ConnectionData::Binary(bin) => {
                                    tracing::debug!("EVM: received binary message: {:x?}", bin);
                                    continue;
                                    // TODO - parse the message and route to appropriate subscription
                                },
                            };

                            match result {
                                LocalResult::Rpc { id, response } => {
                                    // since we clear the rpc ids (and sub ids) on disconnect,
                                    // we can be sure that any new subscription id we get is for this connection
                                    let kind = match id.kind() {
                                        Some(kind) => kind,
                                        None => {
                                            tracing::warn!("EVM: received response for unknown RPC id {}", id.data().as_ffi());
                                            continue;
                                        },
                                    };
                                    match response {
                                        RpcResponse::NewSubscription { subscription_id } => {
                                            match kind {
                                                RpcRequestKind::SubscribeNewHeads => {
                                                    tracing::info!("EVM: subscribed to newHeads with subscription id {}", subscription_id);
                                                    active.ids.insert(subscription_id, SubscriptionKind::NewHeads, None);
                                                },
                                                RpcRequestKind::SubscribeLogs{address, topics} => {
                                                    tracing::info!("EVM: subscribed to logs with subscription id {}", subscription_id);
                                                    active.ids.insert(subscription_id, SubscriptionKind::Logs, Some((address, topics)));
                                                },
                                                RpcRequestKind::SubscribeNewPendingTransactions => {
                                                    tracing::info!("EVM: subscribed to newPendingTransactions with subscription id {}", subscription_id);
                                                    active.ids.insert(subscription_id, SubscriptionKind::NewPendingTransactions, None);
                                                },
                                                RpcRequestKind::Unsubscribe{subscription_id} => {
                                                    tracing::error!("EVM: received newSubscription response for unsubscribe request id {} (subscription id: {})", id.data().as_ffi(), subscription_id);
                                                },
                                            }
                                        },
                                        RpcResponse::UnsubscribeAck(success) => {
                                            if success {
                                                match kind {
                                                    RpcRequestKind::Unsubscribe{subscription_id} => {
                                                        tracing::info!("EVM: unsubscribed from subscription id {}", subscription_id);
                                                        active.ids.remove(&subscription_id);
                                                    },
                                                    _ => {
                                                        tracing::error!("EVM: received unsubscribeAck for non-unsubscribe request id {}", id.data().as_ffi());
                                                    }
                                                }
                                            } else {
                                                match kind {
                                                    RpcRequestKind::Unsubscribe{subscription_id} => {
                                                        tracing::warn!("EVM: failed to unsubscribe from subscription id {}", subscription_id);
                                                    },
                                                    _ => {
                                                        tracing::error!("EVM: received unsubscribeAck for non-unsubscribe request id {}", id.data().as_ffi());
                                                    }
                                                }
                                            }
                                        },
                                        RpcResponse::Other(value) => {
                                            tracing::warn!("EVM: received unexpected RPC response for id {}: {:?}", id.data().as_ffi(), value);
                                        },
                                    }
                                },
                                LocalResult::Subscription { id: subscription_id, event } => match event {
                                    RpcSubscriptionEvent::NewHeads(header) => {
                                        if !active.ids.eq(&subscription_id, SubscriptionKind::NewHeads) {
                                            tracing::warn!("EVM: received newHeads event for unknown subscription id {}", subscription_id);
                                            continue;
                                        }
                                        if let Err(e) = subscription_block_height_tx.send(header.number) {
                                            tracing::error!("EVM: failed to send new block height: {}", e);
                                        }
                                    },
                                    RpcSubscriptionEvent::Logs(log) => {
                                        if !active.ids.eq(&subscription_id, SubscriptionKind::Logs) {
                                            tracing::warn!("EVM: received logs event for unknown subscription id {}", subscription_id);
                                            continue;
                                        }

                                        if let Err(e) = subscription_log_tx.send(log) {
                                            tracing::error!("EVM: failed to send log: {}", e);
                                        }

                                    },
                                    RpcSubscriptionEvent::NewPendingTransaction(tx) => {
                                        if !active.ids.eq(&subscription_id, SubscriptionKind::NewPendingTransactions) {
                                            tracing::warn!("EVM: received newPendingTransaction event for unknown subscription id {}", subscription_id);
                                            continue;
                                        }

                                        if let Err(e) = subscription_new_pending_transaction_tx.send(tx) {
                                            tracing::error!("EVM: failed to send new pending transaction: {}", e);
                                        }

                                    },
                                },
                            }
                        }
                        Some(state) = connection_state_rx.recv() => {
                            match state {
                                ConnectionState::Connected(_endpoint) => {
                                    active.set_is_connected(true);
                                },
                                ConnectionState::Disconnected => {
                                    active.set_is_connected(false);
                                    RpcId::clear_all();
                                },
                            }
                        }
                    }
                }
            }
        });

        Self {
            handle: Arc::new(std::sync::Mutex::new(Some(handle))),
            shutdown_tx: Arc::new(std::sync::Mutex::new(Some(shutdown_tx))),
            active,
        }
    }

    pub fn enable_block_height(&self) {
        self.active.set_blocks(true);
    }

    pub fn enable_log(&self, address: Option<Address>, event: Option<B256>) {
        self.active.insert_log(address, event);
    }

    pub fn enable_pending_transactions(&self) {
        self.active.set_pending_transactions(true);
    }
}

impl Drop for Subscriptions {
    fn drop(&mut self) {
        tracing::debug!("EVM: subscription dropped");
        if let Some(tx) = self.shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }

        if let Some(mut handle) = self.handle.lock().unwrap().take() {
            tokio::spawn(async move {
                if let Err(_) = tokio::time::timeout(Duration::from_millis(500), &mut handle).await
                {
                    tracing::warn!("EVM: subscription loop did not shut down in time, aborting");
                    handle.abort();
                }
            });
        }
    }
}

struct ActiveSubscriptions {
    _blocks: AtomicBool,
    _logs: std::sync::RwLock<LogFilter>,
    _pending_transactions: AtomicBool,
    // not really a subscription, but used to track connection state
    _is_connected: AtomicBool,
    ids: SubscriptionIds,
    connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>,
}

impl ActiveSubscriptions {
    pub fn new(connection_send_rpc_tx: tokio::sync::mpsc::UnboundedSender<RpcRequest>) -> Self {
        Self {
            _blocks: AtomicBool::new(false),
            _logs: std::sync::RwLock::new(LogFilter::default()),
            _pending_transactions: AtomicBool::new(false),
            _is_connected: AtomicBool::new(false),
            ids: SubscriptionIds::default(),
            connection_send_rpc_tx,
        }
    }

    pub fn set_blocks(&self, value: bool) {
        self._blocks
            .store(value, std::sync::atomic::Ordering::SeqCst);

        self.resubscribe_if_connected();
    }

    pub fn insert_log(&self, address: Option<Address>, event: Option<B256>) {
        {
            let mut lock = self._logs.write().unwrap();

            if let Some(address) = address {
                lock.addresses.insert(address);
            }
            if let Some(event) = event {
                lock.events.insert(event);
            }
        }
        self.unsubscribe_logs();
        self.resubscribe_if_connected();
    }
    pub fn set_pending_transactions(&self, value: bool) {
        self._pending_transactions
            .store(value, std::sync::atomic::Ordering::SeqCst);
        self.resubscribe_if_connected();
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

    fn unsubscribe_logs(&self) {
        let ids = self.ids.list(SubscriptionKind::Logs);
        for id in ids {
            if let Err(e) = self
                .connection_send_rpc_tx
                .send(RpcRequest::unsubscribe(id.clone()))
            {
                tracing::error!(
                    "EVM: failed to send unsubscribe request for logs subscription id {}: {}",
                    id,
                    e
                );
            } else {
                tracing::info!(
                    "EVM: sent unsubscribe request for logs subscription id {}",
                    id
                );
            }
            self.ids.remove(&id);
        }
    }

    fn resubscribe_if_connected(&self) {
        if !self._is_connected.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }

        // only ever allow one subscription for blocks/newHeads
        if self._blocks.load(std::sync::atomic::Ordering::SeqCst)
            && !self.ids.any(SubscriptionKind::NewHeads)
        {
            if let Err(e) = self.connection_send_rpc_tx.send(RpcRequest::new_heads()) {
                tracing::error!("EVM: failed to send newHeads subscription request: {}", e);
            } else {
                tracing::info!("EVM: sent newHeads subscription request");
            }
        }

        // logs is a bit tricky, we can have multiple subscriptions for different filters in flight
        // before the stale ones finish unsubscribing
        // so we need to make sure we aren't re-subscribing to a filter we already have
        {
            let (addresses, events) = {
                let lock = self._logs.read().unwrap();
                let addresses: Vec<Address> = lock.addresses.iter().cloned().collect();
                let events: Vec<B256> = lock.events.iter().cloned().collect();

                (addresses, events)
            };

            if (!addresses.is_empty() || !events.is_empty())
                && !self.ids.any_log_filter(&addresses, &events)
            {
                if let Err(e) = self
                    .connection_send_rpc_tx
                    .send(RpcRequest::logs(addresses, events))
                {
                    tracing::error!("EVM: failed to send logs subscription request: {}", e);
                } else {
                    tracing::info!("EVM: sent logs subscription request");
                }
            }
        }

        // only ever allow one subscription for pending transactions
        if self
            ._pending_transactions
            .load(std::sync::atomic::Ordering::SeqCst)
            && !self.ids.any(SubscriptionKind::NewPendingTransactions)
        {
            if let Err(e) = self
                .connection_send_rpc_tx
                .send(RpcRequest::new_pending_transactions())
            {
                tracing::error!(
                    "EVM: failed to send newPendingTransactions subscription request: {}",
                    e
                );
            } else {
                tracing::info!("EVM: sent newPendingTransactions subscription request");
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
        log_filters: Option<(Vec<Address>, Vec<B256>)>,
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

    fn any_log_filter(&self, addresses: &[Address], events: &[B256]) -> bool {
        let lock = self._log_filters.read().unwrap();
        for (addr_set, event_set) in lock.values() {
            if addr_set.len() == addresses.len()
                && event_set.len() == events.len()
                && addr_set.iter().all(|a| addresses.contains(a))
                && event_set.iter().all(|e| events.contains(e))
            {
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
        match self._lookup.read().unwrap().get(id) {
            Some(current_kind) if *current_kind == kind => true,
            _ => false,
        }
    }
}
