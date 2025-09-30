mod connection;
mod types;

use connection::Connection;
use futures::Stream;
use wavs_types::ChainKeyId;

pub struct EvmTriggerClient {
    connection: Connection,
}

impl EvmTriggerClient {
    pub fn new(chain_id: ChainKeyId, ws_endpoints: Vec<String>) -> Self {
        let connection = Connection::new(ws_endpoints, |msg| {});

        Self { connection }
    }

    pub fn stream_block_height(&self) -> impl Stream<Item = u64> {
        // Placeholder implementation
        futures::stream::iter(vec![1, 2, 3, 4, 5, 6])
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
    async fn evm_block_height_stream() {
        init_tracing_tests();

        let anvil = Anvil::new().spawn();

        let client = EvmTriggerClient::new(
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
