use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use tokio::{sync::oneshot, task::JoinHandle};

use crate::subsystems::trigger::clients::evm::{
    channels::SubscriptionChannels,
    connection::{ConnectionData, ConnectionState},
    rpc::{RpcRequest, RpcResponse, RpcResponsePayload, RpcResult},
};

#[derive(Clone)]
pub struct Subscriptions {
    handle: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
    shutdown_tx: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

#[derive(Clone, Default)]
struct SubscriptionIds {
    _new_heads: Arc<std::sync::Mutex<Option<String>>>,
    _logs: Arc<std::sync::Mutex<Option<String>>>,
    _new_pending_transactions: Arc<std::sync::Mutex<Option<String>>>,
}

impl SubscriptionIds {
    fn clear(&self) {
        *self._new_heads.lock().unwrap() = None;
        *self._logs.lock().unwrap() = None;
        *self._new_pending_transactions.lock().unwrap() = None;
    }

    fn set_new_heads(&self, id: String) {
        *self._new_heads.lock().unwrap() = Some(id);
    }

    fn new_heads_eq(&self, id: &str) -> bool {
        match &*self._new_heads.lock().unwrap() {
            Some(ref current_id) if current_id == id => true,
            _ => false,
        }
    }

    fn set_logs(&self, id: String) {
        *self._logs.lock().unwrap() = Some(id);
    }

    fn logs_eq(&self, id: &str) -> bool {
        match &*self._logs.lock().unwrap() {
            Some(ref current_id) if current_id == id => true,
            _ => false,
        }
    }

    fn set_new_pending_transactions(&self, id: String) {
        *self._new_pending_transactions.lock().unwrap() = Some(id);
    }

    fn new_pending_transactions_eq(&self, id: &str) -> bool {
        match &*self._new_pending_transactions.lock().unwrap() {
            Some(ref current_id) if current_id == id => true,
            _ => false,
        }
    }
}

#[derive(Clone, Default)]
struct RpcIds {
    _new_heads: Arc<AtomicUsize>,
    _logs: Arc<AtomicUsize>,
    _new_pending_transactions: Arc<AtomicUsize>,
}

impl RpcIds {
    pub fn clear(&self) {
        self._new_heads
            .store(0, std::sync::atomic::Ordering::SeqCst);
        self._logs.store(0, std::sync::atomic::Ordering::SeqCst);
        self._new_pending_transactions
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set_new_heads(&self, id: usize) {
        self._new_heads
            .store(id, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn new_heads(&self) -> usize {
        self._new_heads.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn set_logs(&self, id: usize) {
        self._logs.store(id, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn logs(&self) -> usize {
        self._logs.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn set_new_pending_transactions(&self, id: usize) {
        self._new_pending_transactions
            .store(id, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn new_pending_transactions(&self) -> usize {
        self._new_pending_transactions
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Subscriptions {
    pub fn new(channels: SubscriptionChannels) -> Self {
        let SubscriptionChannels {
            subscription_block_height_tx,
            connection_send_tx,
            mut connection_state_rx,
            mut connection_data_rx,
        } = channels;

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let sub_ids = SubscriptionIds::default();
        let rpc_ids = RpcIds::default();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        tracing::info!("EVM: shutdown requested, exiting subscription loop");
                        break;
                    }

                    Some(msg) = connection_data_rx.recv() => {
                        let (result, response_id) = match msg {
                            ConnectionData::Text(text) => {
                                match serde_json::from_str::<RpcResponse>(&text) {
                                    Ok(response) => {
                                        match response.id.parse::<usize>() {
                                            Ok(id) => {
                                                match response.payload {
                                                    RpcResponsePayload::Success { result } => {
                                                        (result, id)
                                                    }
                                                    RpcResponsePayload::Error { error } => {
                                                        tracing::error!("EVM: RPC error: code {}, message: {}", error.code, error.message);
                                                        continue;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!("EVM: failed to parse response id {}: {}", response.id, e);
                                                continue;
                                            }
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
                            RpcResult::SubscriptionId(subscription_id) => {
                                if response_id == rpc_ids.new_heads() {
                                    tracing::info!("EVM: subscribed to newHeads with subscription id {}", subscription_id);
                                    sub_ids.set_new_heads(subscription_id);
                                } else if response_id == rpc_ids.logs() {
                                    tracing::info!("EVM: subscribed to logs with subscription id {}", subscription_id);
                                    sub_ids.set_logs(subscription_id);
                                } else if response_id == rpc_ids.new_pending_transactions() {
                                    tracing::info!("EVM: subscribed to newPendingTransactions with subscription id {}", subscription_id);
                                    sub_ids.set_new_pending_transactions(subscription_id);
                                } else {
                                    tracing::warn!("EVM: received unknown subscription id {} for response id {}", subscription_id, response_id);
                                }
                            },
                            RpcResult::UnsubscribeSuccess(_) => {

                            },
                            RpcResult::SubscriptionData { subscription, result } => {
                                if sub_ids.new_heads_eq(&subscription) {

                                    // Handle new block header
                                    // TODO - deserialize the block header and extract the block number
                                    if let Err(e) = subscription_block_height_tx.send(42) {
                                        tracing::error!("EVM: failed to send new block height: {}", e);
                                    }
                                    tracing::info!("Got new block header!")
                                } else if sub_ids.logs_eq(&subscription) {
                                    // Handle log event
                                    tracing::info!("EVM: received log event: {:?}", result);
                                } else if sub_ids.new_pending_transactions_eq(&subscription) {
                                    // Handle new pending transaction
                                    tracing::info!("EVM: received new pending transaction: {:?}", result);
                                } else {
                                    tracing::warn!("EVM: received data for unknown subscription id {}", subscription);
                                }

                            },
                        }
                        // Handle incoming messages and route them to the appropriate subscription
                        // like maybe we get a new block height and need to send it to subscription_block_height_tx
                    }
                    Some(state) = connection_state_rx.recv() => {
                        match state {
                            ConnectionState::Connected(_endpoint) => {
                                tracing::info!("EVM connected on {}", _endpoint);
                                let req = RpcRequest::new_heads();
                                rpc_ids.set_new_heads(req.id());
                                if let Err(e) = connection_send_tx.send(req) {
                                    tracing::error!("EVM: failed to send newHeads subscription request: {}", e);
                                } else {
                                    tracing::info!("EVM: sent newHeads subscription request");
                                }

                               // TODO - resubscribe to all active event logs
                            },
                            ConnectionState::Disconnected => {
                                sub_ids.clear();
                                rpc_ids.clear();
                            },
                        }
                    }
                }
            }
        });

        Self {
            handle: Arc::new(std::sync::Mutex::new(Some(handle))),
            shutdown_tx: Arc::new(std::sync::Mutex::new(Some(shutdown_tx))),
        }
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
