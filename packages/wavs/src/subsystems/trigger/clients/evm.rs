// TODO - fix nonce too low issues
// fix advanced tests
// maintain list of "to unsubscribe" for once it lands
// - and need to test both cases of unsubscribing (before and after it lands)
mod channels;
mod connection;
mod rpc_types;
mod subscription;

use alloy_primitives::B256;
use alloy_rpc_types_eth::Log;
use connection::Connection;
use futures::Stream;
use subscription::Subscriptions;

use tokio_stream::wrappers::UnboundedReceiverStream;

use channels::Channels;

pub struct EvmTriggerStreams {
    pub controller: EvmTriggerStreamsController,
    pub block_height_stream: UnboundedReceiverStream<u64>,
    pub log_stream: UnboundedReceiverStream<Log>,
    pub new_pending_transaction_stream: UnboundedReceiverStream<B256>,
}

pub struct EvmTriggerStreamsController {
    pub subscriptions: Subscriptions,
    pub connection: Connection,
}

impl EvmTriggerStreams {
    pub fn new(ws_endpoints: Vec<String>) -> Self {
        let channels = Channels::new();

        let subscriptions = Subscriptions::new(channels.subscription);

        let connection = Connection::new(ws_endpoints, channels.connection);

        Self {
            controller: EvmTriggerStreamsController {
                connection,
                subscriptions,
            },
            block_height_stream: UnboundedReceiverStream::new(
                channels.client.subscription_block_height_rx,
            ),
            log_stream: UnboundedReceiverStream::new(channels.client.subscription_log_rx),
            new_pending_transaction_stream: UnboundedReceiverStream::new(
                channels.client.subscription_new_pending_transaction_rx,
            ),
        }
    }
}
