// TODO - make subscription ids a lookup map
// - in theory we should only have one at a time, but we might end up unsubscribing while a subscribe is also in flight, and we don't want to miss any inbetween
// TODO - event logs
// - imperative setting filter
// - remember filter for re-subscribe/re-connect
// - test that it works across filter changes
// - test that it works across re-connects
mod channels;
mod connection;
mod rpc;
mod subscription;

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

impl Drop for EvmTriggerClient {
    fn drop(&mut self) {
        tracing::debug!("EVM: client dropped");
    }
}

#[cfg(test)]
mod test {
    use crate::init_tracing_tests;

    use super::*;
    use futures::StreamExt;
    use tokio::time::{timeout, Duration};
    use utils::test_utils::anvil::safe_spawn_anvil_extra;

    #[tokio::test]
    async fn client_blocks() {
        init_tracing_tests();

        let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

        let mut client = EvmTriggerClient::new(
            anvil.chain_id().to_string().parse().unwrap(),
            vec![anvil.ws_endpoint()],
        );

        let mut stream = client.stream_block_height();

        let mut collected_heights = Vec::new();

        const BLOCKS_TO_COLLECT: usize = 5;
        timeout(Duration::from_secs(5), async {
            while let Some(height) = stream.next().await {
                collected_heights.push(height);
                if collected_heights.len() >= BLOCKS_TO_COLLECT {
                    break;
                }
            }
        })
        .await
        .unwrap();

        assert!(
            collected_heights.len() >= BLOCKS_TO_COLLECT,
            "only got {} blocks, not enough to test",
            collected_heights.len()
        );

        // assert that the block heights are sequential
        for window in collected_heights.windows(2) {
            assert_eq!(window[1], window[0] + 1, "Block heights are not sequential");
        }
    }
}
