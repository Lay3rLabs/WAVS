use std::{
    sync::{atomic::AtomicUsize, Arc},
    time::Duration,
};

use tokio::{sync::oneshot, task::JoinHandle};

use crate::subsystems::trigger::clients::evm::{
    channels::SubscriptionChannels, connection::ConnectionData,
};

#[derive(Clone)]
pub struct Subscriptions {
    handle: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
    shutdown_tx: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
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

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        tracing::info!("EVM: shutdown requested, exiting subscription loop");
                        break;
                    }

                    Some(msg) = connection_data_rx.recv() => {
                        // Handle incoming messages and route them to the appropriate subscription
                        // like maybe we get a new block height and need to send it to subscription_block_height_tx
                    }
                    Some(state) = connection_state_rx.recv() => {
                        match state {
                            ConnectionState::Connected(_endpoint) => {
                               // TODO - resubscribe to all active subscriptions (send on connection_send_tx)
                            },
                            ConnectionState::Disconnected => {
                               // TODO - idk, maybe nothing?
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
