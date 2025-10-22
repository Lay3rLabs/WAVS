mod channels;
mod connection;
mod rpc_types;
mod subscription;

use alloy_primitives::B256;
use alloy_rpc_types_eth::Log;
use connection::Connection;
use subscription::Subscriptions;

// just for debug/http visibility
pub use subscription::SubscriptionKind;

use tokio_stream::wrappers::UnboundedReceiverStream;

use channels::Channels;

use crate::subsystems::trigger::streams::evm_stream::client::rpc_types::id::RpcIds;

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
    pub fn new(
        ws_endpoints: Vec<String>,
        chain_key: wavs_types::ChainKey,
        ws_priority_endpoint_index: Option<usize>,
    ) -> Self {
        let channels = Channels::new();

        let rpc_ids = RpcIds::new();

        let subscriptions = Subscriptions::new(rpc_ids.clone(), channels.subscription);

        let connection = Connection::new(
            rpc_ids,
            ws_endpoints,
            channels.connection,
            chain_key,
            ws_priority_endpoint_index,
        );

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
