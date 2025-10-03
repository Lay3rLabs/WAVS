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
async fn debug_multiple_enable_log_calls() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);
    let contract = EventEmitterClient::new(&anvil).deploy().await;

    tracing::info!("=== DEBUG: Deployed contract at {} ===", contract.address());

    // Enable first event type
    tracing::info!("=== DEBUG: Enabling IntegerEvent ===");
    tracing::info!(
        "=== DEBUG: IntegerEvent hash: {:?} ===",
        EventEmitter::IntegerEvent::SIGNATURE_HASH
    );
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );

    // Add delay to see if timing matters
    tracing::info!("=== DEBUG: Waiting after first enable_log ===");
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Enable second event type
    tracing::info!("=== DEBUG: Enabling StringEvent ===");
    tracing::info!(
        "=== DEBUG: StringEvent hash: {:?} ===",
        EventEmitter::StringEvent::SIGNATURE_HASH
    );
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::StringEvent::SIGNATURE_HASH),
    );

    // Add delay to let subscriptions settle
    tracing::info!("=== DEBUG: Waiting after second enable_log ===");
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    let mut stream = client.stream_log();
    tracing::info!("=== DEBUG: Created stream ===");

    // Emit events
    tracing::info!("=== DEBUG: Emitting events ===");
    let _ = contract.emitInteger(U256::from(42)).send().await.unwrap();
    let _ = contract
        .emitString("test".to_string())
        .send()
        .await
        .unwrap();

    // Collect events with short timeout
    let mut events_received = 0;
    let timeout_result = tokio::time::timeout(Duration::from_millis(500), async {
        while let Some(log) = stream.next().await {
            events_received += 1;
            tracing::info!("=== DEBUG: Received event #{} ===", events_received);
            if events_received >= 2 {
                break;
            }
        }
    })
    .await;

    if events_received >= 2 {
        tracing::info!(
            "=== DEBUG: SUCCESS! Received {} events ===",
            events_received
        );
    } else {
        tracing::error!("=== DEBUG: Only received {} events ===", events_received);
        panic!("Expected at least 2 events but got {}", events_received);
    }
}

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
async fn client_logs_multiple_events_same_contract() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);
    let contract = EventEmitterClient::new(&anvil).deploy().await;

    // Enable both integer and string events for the same contract
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::StringEvent::SIGNATURE_HASH),
    );

    let mut stream = client.stream_log();

    const LOGS_TO_COLLECT: usize = 4;
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Collect logs in background
    let collect_handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(10), async {
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

    // Emit both integer and string events sequentially (no watch needed for same contract)
    for i in 0..2 {
        let _ = contract.emitInteger(U256::from(i)).send().await.unwrap();
        let _ = contract
            .emitString(format!("test_{}", i))
            .send()
            .await
            .unwrap();
    }

    collect_handle.await.unwrap();

    let logs = collected_logs.lock().unwrap();
    assert_eq!(
        logs.len(),
        LOGS_TO_COLLECT,
        "Should collect both integer and string events"
    );
    tracing::info!(
        "✓ Collected {} events from single contract with multiple event types",
        logs.len()
    );
}

#[tokio::test]
async fn client_logs_multiple_contracts() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let contract = EventEmitterClient::new(&anvil).deploy().await;
    let contract2 = EventEmitterClient::new(&anvil).deploy().await;

    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

    // Enable logs for both contracts
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );
    client.enable_log(
        Some(contract2.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );

    let mut stream = client.stream_log();
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Collect logs from both contracts
    let collect_handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(10), async {
                while let Some(log) = stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    lock.push(log);
                    if lock.len() >= 2 {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Emit from both contracts
    let _ = contract
        .emitInteger(U256::from(100))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
    let _ = contract2
        .emitInteger(U256::from(200))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    collect_handle.await.unwrap();

    let logs = collected_logs.lock().unwrap();
    assert_eq!(logs.len(), 2, "Should collect events from both contracts");

    // Verify we got events from both contracts
    let mut addresses = HashSet::new();
    for log in logs.iter() {
        addresses.insert(log.inner.address);
    }
    assert_eq!(
        addresses.len(),
        2,
        "Should have events from 2 different contracts"
    );
    assert!(
        addresses.contains(contract.address()),
        "Should have events from contract 1"
    );
    assert!(
        addresses.contains(contract2.address()),
        "Should have events from contract 2"
    );
    tracing::info!(
        "✓ Collected events from {} different contracts",
        addresses.len()
    );
}

#[tokio::test]
async fn client_logs_multiple_rpc_endpoints() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let anvil2 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let contract = EventEmitterClient::new(&anvil).deploy().await;
    let contract2 = EventEmitterClient::new(&anvil2).deploy().await;

    // Create client with multiple endpoints
    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint(), anvil2.ws_endpoint()]);

    // Enable logs for contracts on both chains
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );
    client.enable_log(
        Some(contract2.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );

    let mut stream = client.stream_log();
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Collect logs from both chains
    let collect_handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(10), async {
                while let Some(log) = stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    lock.push(log);
                    if lock.len() >= 2 {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Emit from contracts on different chains
    let _ = contract
        .emitInteger(U256::from(300))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
    let _ = contract2
        .emitInteger(U256::from(400))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    collect_handle.await.unwrap();

    let logs = collected_logs.lock().unwrap();
    assert_eq!(
        logs.len(),
        2,
        "Should collect events from contracts on different chains"
    );

    // Verify we got events from both contracts
    let mut addresses = HashSet::new();
    for log in logs.iter() {
        addresses.insert(log.inner.address);
    }
    assert_eq!(
        addresses.len(),
        2,
        "Should have events from 2 different contracts"
    );
    assert!(
        addresses.contains(contract.address()),
        "Should have events from contract on chain 1"
    );
    assert!(
        addresses.contains(contract2.address()),
        "Should have events from contract on chain 2"
    );
    tracing::info!(
        "✓ Collected events from contracts on {} different chains",
        addresses.len()
    );
}

#[tokio::test]
async fn client_logs_dynamic_event_addition() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let contract = EventEmitterClient::new(&anvil).deploy().await;

    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

    // Start with just integer events
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::IntegerEvent::SIGNATURE_HASH),
    );

    // Then add string events for the same contract
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::StringEvent::SIGNATURE_HASH),
    );

    let mut stream = client.stream_log();
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Collect both event types
    let collect_handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(10), async {
                while let Some(log) = stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    lock.push(log);
                    if lock.len() >= 2 {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Emit both event types
    let _ = contract
        .emitInteger(U256::from(500))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
    let _ = contract
        .emitString("dynamic_test".to_string())
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    collect_handle.await.unwrap();

    let logs = collected_logs.lock().unwrap();
    assert_eq!(
        logs.len(),
        2,
        "Should collect both integer and string events from same contract"
    );

    // Verify we got both event types from the same contract
    let mut event_signatures = HashSet::new();
    for log in logs.iter() {
        event_signatures.insert(log.inner.topics()[0]);
        assert_eq!(
            log.inner.address,
            *contract.address(),
            "All events should be from the same contract"
        );
    }
    assert_eq!(
        event_signatures.len(),
        2,
        "Should have 2 different event types"
    );
    assert!(
        event_signatures.contains(&EventEmitter::IntegerEvent::SIGNATURE_HASH),
        "Should have integer event"
    );
    assert!(
        event_signatures.contains(&EventEmitter::StringEvent::SIGNATURE_HASH),
        "Should have string event"
    );
    tracing::info!(
        "✓ Successfully collected {} different event types from the same contract",
        event_signatures.len()
    );
}

#[tokio::test]
async fn client_logs_string_events() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let mut client = EvmTriggerClient::new(vec![anvil.ws_endpoint()]);

    let contract = EventEmitterClient::new(&anvil).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());
    client.enable_log(
        Some(contract.address().clone()),
        Some(EventEmitter::StringEvent::SIGNATURE_HASH),
    );

    let mut stream = client.stream_log();

    const LOGS_TO_COLLECT: usize = 3;

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

    for i in 0..LOGS_TO_COLLECT {
        let _ = contract
            .emitString(format!("test_string_{}", i))
            .send()
            .await
            .unwrap()
            .watch()
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
        let event = EventEmitter::StringEvent::decode_log(&log.inner)
            .unwrap()
            .data;

        collected_values.insert(event.value.clone());
    }

    for i in 0..LOGS_TO_COLLECT {
        let expected_value = format!("test_string_{}", i);
        assert!(
            collected_values.contains(&expected_value),
            "did not find emitted value {} in logs",
            expected_value
        );

        tracing::info!("found {} in events!", expected_value)
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
