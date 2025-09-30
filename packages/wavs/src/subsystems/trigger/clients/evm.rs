mod channels;
mod connection;
mod subscription;
mod types;

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
}

impl EvmTriggerClient {
    pub fn new(chain_id: ChainKeyId, ws_endpoints: Vec<String>) -> Self {
        let channels = Channels::new();

        let subscriptions = Subscriptions::new(channels.subscription);

        let connection = Connection::new(ws_endpoints, channels.connection);

        Self {
            connection,
            subscriptions,
            block_height_rx: Some(channels.client.subscription_block_height_rx),
        }
    }

    pub fn stream_block_height(&mut self) -> impl Stream<Item = u64> {
        let rx = self
            .block_height_rx
            .take()
            .expect("stream_block_height can only be called once");
        UnboundedReceiverStream::new(rx)
    }
}

#[cfg(test)]
mod test {
    use crate::init_tracing_tests;

    use super::*;
    use alloy_node_bindings::Anvil;
    use futures::StreamExt;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    #[ignore = "wip"]
    async fn evm_block_height_stream() {
        init_tracing_tests();

        let anvil = Anvil::new().spawn();

        let mut client = EvmTriggerClient::new(
            anvil.chain_id().to_string().parse().unwrap(),
            vec![anvil.ws_endpoint()],
        );

        let mut stream = client.stream_block_height();

        let mut collected_heights = Vec::new();

        timeout(Duration::from_secs(5), async {
            while let Some(height) = stream.next().await {
                collected_heights.push(height);
            }
        })
        .await
        .unwrap();

        assert!(
            collected_heights.len() > 5,
            "only got {} blocks, not enough to test",
            collected_heights.len()
        );

        // assert that the block heights are sequential
        for window in collected_heights.windows(2) {
            assert_eq!(window[1], window[0] + 1, "Block heights are not sequential");
        }
    }
}
