// TODO - event logs
// - imperative setting filter
// - remember filter for re-subscribe/re-connect
// - test that it works across filter changes
// - test that it works across re-connects
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

#[cfg(test)]
mod test {
    use std::{collections::HashSet, sync::Arc};

    use crate::{
        init_tracing_tests,
        subsystems::trigger::clients::evm::test::EventEmitter::EventEmitterInstance,
    };

    use super::*;
    use alloy_node_bindings::AnvilInstance;
    use alloy_primitives::U256;
    use alloy_provider::{DynProvider, ProviderBuilder};
    use alloy_signer_local::PrivateKeySigner;
    use alloy_sol_types::{sol, SolEvent};
    use futures::StreamExt;
    use tokio::time::{timeout, Duration};
    use utils::test_utils::anvil::safe_spawn_anvil_extra;

    #[tokio::test]
    async fn client_blocks() {
        init_tracing_tests();

        let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

        let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

        let mut stream = client.stream_block_height();

        client.enable_block_height();

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

    #[tokio::test]
    async fn client_logs() {
        init_tracing_tests();

        let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

        let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

        let contract = EventEmitterClient::new(&anvil).deploy().await;

        tracing::info!("Deployed contract at {}", contract.instance.address());
        client.enable_log(
            Some(contract.instance.address().clone()),
            Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
        );

        let mut stream = client.stream_log();

        const LOGS_TO_COLLECT: usize = 5;

        let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

        let handle = tokio::spawn({
            let collected_logs = collected_logs.clone();
            async move {
                timeout(Duration::from_secs(5), async {
                    while let Some(log) = stream.next().await {
                        let mut lock = collected_logs.lock().unwrap();
                        lock.push(log);
                        if lock.len() >= LOGS_TO_COLLECT {
                            break;
                        }
                    }
                })
                .await
                .unwrap();
            }
        });

        let contract = EventEmitterClient::new(&anvil).deploy().await;

        client.enable_log(
            Some(contract.instance.address().clone()),
            Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
        );

        for value in 0..LOGS_TO_COLLECT as u64 {
            contract.emit_integer(value).await;
        }

        handle.await.unwrap();

        let collected_logs = collected_logs.lock().unwrap();

        assert!(
            collected_logs.len() >= LOGS_TO_COLLECT,
            "only got {} logs, not enough to test",
            collected_logs.len()
        );

        let mut collected_values = HashSet::new();
        for log in collected_logs.iter() {
            let event = EventEmitter::IntegerEvent::decode_log(&log.inner)
                .unwrap()
                .data;

            collected_values.insert(event.value);
        }

        for value in 0..LOGS_TO_COLLECT as u64 {
            assert!(
                collected_values.contains(&U256::from(value)),
                "did not find emitted value {} in logs",
                value
            );
        }
    }

    struct EventEmitterClient {
        provider: DynProvider,
    }

    impl EventEmitterClient {
        pub fn new(anvil: &AnvilInstance) -> Self {
            let wallet = PrivateKeySigner::from_signing_key(anvil.keys()[0].clone().into());

            let provider = ProviderBuilder::new()
                .wallet(wallet)
                .connect_http(anvil.endpoint().parse().unwrap());

            let provider = DynProvider::new(provider);

            Self { provider }
        }

        pub async fn deploy(&self) -> EventEmitterContract {
            let instance = EventEmitter::deploy(self.provider.clone()).await.unwrap();

            EventEmitterContract { instance }
        }
    }

    struct EventEmitterContract {
        instance: EventEmitterInstance<DynProvider>,
    }

    impl EventEmitterContract {
        pub async fn emit_integer(&self, value: u64) {
            let _tx = self
                .instance
                .emitInteger(U256::from(value))
                .send()
                .await
                .unwrap();
        }

        pub async fn emit_string(&self, value: impl ToString) {
            let _tx = self
                .instance
                .emitString(value.to_string())
                .send()
                .await
                .unwrap();
        }
    }

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        EventEmitter,
        "./tests/contracts/solidity/abi/EventEmitter.sol/EventEmitter.json"
    );
}
