// TODO - fix nonce too low issues
// fix advanced tests
mod channels;
mod connection;
mod rpc_types;
mod subscription;

use alloy_primitives::{Address, B256};
use alloy_rpc_types_eth::Log;
use connection::Connection;
use futures::Stream;
use subscription::Subscriptions;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_stream::wrappers::UnboundedReceiverStream;
use wavs_types::ChainKeyId;

use channels::Channels;

pub struct EvmTriggerClient {
    connection: Connection,
    subscriptions: Subscriptions,
    block_height_rx: Option<UnboundedReceiver<u64>>,
    log_rx: Option<UnboundedReceiver<Log>>,
    new_pending_transaction_rx: Option<UnboundedReceiver<B256>>,
}

impl EvmTriggerClient {
    pub fn new(ws_endpoints: Vec<String>) -> Self {
        let channels = Channels::new();

        let subscriptions = Subscriptions::new(channels.subscription);

        let connection = Connection::new(ws_endpoints, channels.connection);

        Self {
            connection,
            subscriptions,
            block_height_rx: Some(channels.client.subscription_block_height_rx),
            log_rx: Some(channels.client.subscription_log_rx),
            new_pending_transaction_rx: Some(
                channels.client.subscription_new_pending_transaction_rx,
            ),
        }
    }

    // meh, for now we only enable - no need to disable
    pub fn enable_block_height(&self) {
        self.subscriptions.enable_block_height();
    }

    pub fn enable_log(&self, address: Option<Address>, event: Option<B256>) {
        self.subscriptions.enable_log(address, event);
    }

    pub fn enable_pending_transactions(&self) {
        self.subscriptions.enable_pending_transactions();
    }

    // only call these once
    pub fn stream_block_height(&mut self) -> impl Stream<Item = u64> {
        let rx = self
            .block_height_rx
            .take()
            .expect("stream_block_height can only be called once");
        UnboundedReceiverStream::new(rx)
    }

    pub fn stream_log(&mut self) -> impl Stream<Item = Log> {
        let rx = self
            .log_rx
            .take()
            .expect("stream_log can only be called once");
        UnboundedReceiverStream::new(rx)
    }

    pub fn stream_new_pending_transactions(&mut self) -> impl Stream<Item = B256> {
        let rx = self
            .new_pending_transaction_rx
            .take()
            .expect("stream_new_pending_transactions can only be called once");
        UnboundedReceiverStream::new(rx)
    }
}

impl Drop for EvmTriggerClient {
    fn drop(&mut self) {
        tracing::debug!("EVM: client dropped");
    }
}
