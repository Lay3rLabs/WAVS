use std::{sync::Arc, time::Duration};

use slotmap::Key;
use tokio::{sync::oneshot, task::JoinHandle};

use crate::subsystems::trigger::clients::evm::{
    channels::SubscriptionChannels,
    connection::{ConnectionData, ConnectionState},
    rpc::{
        id::{RpcId, RpcRequestKind},
        inbound::{RpcInbound, RpcResponse, RpcSubscriptionEvent},
        outbound::RpcRequest,
    },
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

        let handle = tokio::spawn(async move {
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
                                                sub_ids.set_new_heads(subscription_id);
                                            },
                                            RpcRequestKind::SubscribeLogs => {
                                                tracing::info!("EVM: subscribed to logs with subscription id {}", subscription_id);
                                                sub_ids.set_logs(subscription_id);
                                            },
                                            RpcRequestKind::SubscribeNewPendingTransactions => {
                                                tracing::info!("EVM: subscribed to newPendingTransactions with subscription id {}", subscription_id);
                                                sub_ids.set_new_pending_transactions(subscription_id);
                                            },
                                            RpcRequestKind::Unsubscribe => {
                                                tracing::error!("EVM: received newSubscription response for unsubscribe request id {}", id.data().as_ffi());
                                            },
                                        }
                                    },
                                    RpcResponse::UnsubscribeAck(success) => {

                                    },
                                    RpcResponse::Other(value) => {

                                    },
                                }
                            },
                            LocalResult::Subscription { id: subscription_id, event } => match event {
                                RpcSubscriptionEvent::NewHeads(header) => {
                                    if !sub_ids.new_heads_eq(&subscription_id) {
                                        tracing::warn!("EVM: received newHeads event for unknown subscription id {}", subscription_id);
                                        continue;
                                    }
                                    if let Err(e) = subscription_block_height_tx.send(header.number) {
                                        tracing::error!("EVM: failed to send new block height: {}", e);
                                    }
                                    tracing::debug!("Got new block header ({})!", header.number)

                                },
                                RpcSubscriptionEvent::Logs(log) => {
                                    if !sub_ids.logs_eq(&subscription_id) {
                                        tracing::warn!("EVM: received logs event for unknown subscription id {}", subscription_id);
                                        continue;
                                    }

                                },
                                RpcSubscriptionEvent::NewPendingTransaction(fixed_bytes) => {
                                    if !sub_ids.new_pending_transactions_eq(&subscription_id) {
                                        tracing::warn!("EVM: received newPendingTransaction event for unknown subscription id {}", subscription_id);
                                        continue;
                                    }
                                },
                            },
                        }
                    }
                    Some(state) = connection_state_rx.recv() => {
                        match state {
                            ConnectionState::Connected(_endpoint) => {
                                tracing::info!("EVM connected on {}", _endpoint);
                                if let Err(e) = connection_send_tx.send(RpcRequest::new_heads()) {
                                    tracing::error!("EVM: failed to send newHeads subscription request: {}", e);
                                } else {
                                    tracing::info!("EVM: sent newHeads subscription request");
                                }

                               // TODO - resubscribe to all active event logs
                            },
                            ConnectionState::Disconnected => {
                                sub_ids.clear();
                                RpcId::clear_all();
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
