use std::{collections::HashSet, sync::Arc};

use crate::{init_tracing_tests, subsystems::trigger::clients::evm::EvmTriggerClient};

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

    tracing::info!("Deployed contract at {}", contract.address());
    client.enable_log(
        Some(contract.address().clone()),
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
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );

    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
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

        tracing::info!("found {value} in events!")
    }
}

#[tokio::test]
async fn client_logs_advanced() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

    let contract = EventEmitterClient::new(&anvil).deploy().await;
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

    pub async fn deploy(&self) -> EventEmitter::EventEmitterInstance<DynProvider> {
        EventEmitter::deploy(self.provider.clone()).await.unwrap()
    }
}

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    EventEmitter,
    "./tests/contracts/solidity/abi/EventEmitter.sol/EventEmitter.json"
);
