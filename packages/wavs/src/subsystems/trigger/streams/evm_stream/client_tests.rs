// Tip, run like: RUST_LOG=info cargo test evm_stream::client_tests::{SINGLE_TEST} --lib -- --nocapture
// e.g. RUST_LOG=info cargo test evm_stream::client_tests::block_height_stream --lib -- --nocapture

mod helpers;

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    },
};

use crate::{
    init_tracing_tests,
    subsystems::trigger::streams::evm_stream::{
        client::EvmTriggerStreams,
        client_tests::helpers::{
            wait_for_all_rpc_requests_landed, EventEmitter, EventEmitterClient,
        },
    },
};

use alloy_primitives::U256;
use alloy_sol_types::SolEvent;
use futures::StreamExt;
use tokio::time::{timeout, Duration};
use utils::test_utils::anvil::safe_spawn_anvil_extra;

#[tokio::test]
async fn block_height_stream() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut block_height_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    controller.subscriptions.toggle_block_height(true);

    let mut collected_heights = Vec::new();

    const BLOCKS_TO_COLLECT: usize = 5;
    timeout(Duration::from_secs(5), async {
        while let Some(height) = block_height_stream.next().await {
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
async fn simple_log_stream() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract = EventEmitterClient::new(&anvil, 0).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());

    controller.subscriptions.enable_logs(
        vec![*contract.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    const LOGS_TO_COLLECT: usize = 5;
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream.next().await {
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

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();

    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    // Extract the events
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
async fn multi_batch_add_log_stream() {
    multi_log_stream(AddKind::Batch).await;
}

#[tokio::test]
async fn multi_serial_add_log_stream() {
    multi_log_stream(AddKind::Serial).await;
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum AddKind {
    Batch,
    Serial,
}

async fn multi_log_stream(add_kind: AddKind) {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract = EventEmitterClient::new(&anvil, 0).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());

    match add_kind {
        AddKind::Batch => controller.subscriptions.enable_logs(
            vec![*contract.address()],
            vec![
                EventEmitter::IntegerEvent::SIGNATURE_HASH,
                EventEmitter::StringEvent::SIGNATURE_HASH,
            ],
        ),
        AddKind::Serial => {
            controller.subscriptions.enable_logs(
                vec![*contract.address()],
                vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
            );
            controller.subscriptions.enable_logs(
                vec![*contract.address()],
                vec![EventEmitter::StringEvent::SIGNATURE_HASH],
            );
        }
    }

    wait_for_all_rpc_requests_landed(&controller).await;

    const LOGS_TO_COLLECT: usize = 6;

    #[derive(Default)]
    struct LogsToCollect {
        integers: HashSet<U256>,
        strings: HashSet<String>,
    }

    impl LogsToCollect {
        fn len(&self) -> usize {
            self.integers.len() + self.strings.len()
        }
    }
    let collected_logs = Arc::new(std::sync::Mutex::new(LogsToCollect::default()));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    match EventEmitter::IntegerEvent::decode_log(&log.inner) {
                        Ok(event) => {
                            tracing::info!("got integer: {}", event.data.value);
                            lock.integers.insert(event.data.value);
                        }
                        Err(_) => {
                            // Not an integer event, try string event
                            match EventEmitter::StringEvent::decode_log(&log.inner) {
                                Ok(event) => {
                                    tracing::info!("got string: {}", event.data.value);
                                    lock.strings.insert(event.data.value);
                                }
                                Err(e) => {
                                    panic!("Failed to decode log as either IntegerEvent or StringEvent: {e}");
                                }
                            }
                        }
                    }

                    if lock.len() >= LOGS_TO_COLLECT {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT as u64 {
        if value % 2 == 0 {
            let _ = contract
                .emitString(format!("test_string_{}", value))
                .send()
                .await
                .unwrap();
        } else {
            let _ = contract
                .emitInteger(U256::from(value))
                .send()
                .await
                .unwrap();
        }
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();

    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    for i in 0..LOGS_TO_COLLECT as u64 {
        if i % 2 == 0 {
            let expected_value = format!("test_string_{}", i);
            assert!(
                collected_logs.strings.contains(&expected_value),
                "did not find {} in string logs",
                expected_value
            );

            tracing::info!("found string event {expected_value} in string events!");
        } else {
            assert!(
                collected_logs.integers.contains(&U256::from(i)),
                "did not find {} in integer logs",
                i
            );

            tracing::info!("found {i} in integer events!")
        }
    }
}

#[tokio::test]
async fn multi_contract_log_stream() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract_1 = EventEmitterClient::new(&anvil, 0).deploy().await;
    tracing::info!("Deployed contract_1 at {}", contract_1.address());

    let contract_2 = EventEmitterClient::new(&anvil, 1).deploy().await;
    tracing::info!("Deployed contract_2 at {}", contract_2.address());

    controller.subscriptions.enable_logs(
        vec![*contract_1.address(), *contract_2.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    wait_for_all_rpc_requests_landed(&controller).await;

    const LOGS_TO_COLLECT: usize = 6;

    #[derive(Default)]
    struct LogsToCollect {
        contract_1_values: HashSet<U256>,
        contract_2_values: HashSet<U256>,
    }

    impl LogsToCollect {
        fn len(&self) -> usize {
            self.contract_1_values.len() + self.contract_2_values.len()
        }
    }

    let collected_logs = Arc::new(std::sync::Mutex::new(LogsToCollect::default()));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        let contract_1_address = *contract_1.address();
        let contract_2_address = *contract_2.address();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    match EventEmitter::IntegerEvent::decode_log(&log.inner) {
                        Ok(event) => {
                            tracing::info!("got integer: {}", event.data.value);
                            if log.inner.address == contract_1_address {
                                lock.contract_1_values.insert(event.data.value);
                            } else if log.inner.address == contract_2_address {
                                lock.contract_2_values.insert(event.data.value);
                            } else {
                                panic!("Received log from unknown contract: {}", log.inner.address);
                            }
                        }
                        Err(e) => {
                            panic!("Failed to decode log as IntegerEvent: {e}");
                        }
                    }

                    if lock.len() >= LOGS_TO_COLLECT {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT {
        if value % 2 == 0 {
            let _ = contract_1
                .emitInteger(U256::from(value as u64))
                .send()
                .await
                .unwrap();
        } else {
            let _ = contract_2
                .emitInteger(U256::from(value as u64))
                .send()
                .await
                .unwrap();
        }
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();
    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    for i in 0..LOGS_TO_COLLECT {
        if i % 2 == 0 {
            assert!(
                collected_logs
                    .contract_1_values
                    .contains(&U256::from(i as u64)),
                "did not find {} in contract_1 logs",
                i
            );

            tracing::info!("found {i} in contract_1 events!")
        } else {
            assert!(
                collected_logs
                    .contract_2_values
                    .contains(&U256::from(i as u64)),
                "did not find {} in contract_2 logs",
                i
            );

            tracing::info!("found {i} in contract_2 events!")
        }
    }
}

#[tokio::test]
async fn fallback_chain_log_stream() {
    init_tracing_tests();

    let anvil_1 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let anvil_2 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil_1.ws_endpoint(), anvil_2.ws_endpoint()], None);

    let contract_1 = EventEmitterClient::new(&anvil_1, 0).deploy().await;
    tracing::info!(
        "Deployed contract_1 at {} (chain port: {})",
        contract_1.address(),
        anvil_1.port()
    );
    let contract_2 = EventEmitterClient::new(&anvil_2, 0).deploy().await;
    tracing::info!(
        "Deployed contract_2 at {} (chain port: {})",
        contract_2.address(),
        anvil_2.port()
    );

    controller.subscriptions.enable_logs(
        vec![*contract_1.address(), *contract_2.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    wait_for_all_rpc_requests_landed(&controller).await;

    const LOGS_TO_COLLECT: usize = 4;
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));
    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(10), async {
                while let Some(log) = log_stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    if let Ok(event) = EventEmitter::IntegerEvent::decode_log(&log.inner) {
                        lock.push(event);
                    }
                    if lock.len() >= LOGS_TO_COLLECT {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // kill anvil_1 to force fallback to anvil_2
    drop(anvil_1);

    // wait for the client to reconnect (otherwise our subscription will just miss the events)
    timeout(Duration::from_secs(10), async {
        loop {
            if controller.connection.current_endpoint() == Some(anvil_2.ws_endpoint())
                && controller.subscriptions.is_connected()
            {
                wait_for_all_rpc_requests_landed(&controller).await;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Now we can emit the events
    // only on contract_2
    // tip: commenting out the drop-and-wait above will cause the test to fail since the client will be connected to anvil_1

    for value in 0..LOGS_TO_COLLECT {
        let _ = contract_2
            .emitInteger(U256::from(value as u64))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish

    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();
    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    for i in 0..LOGS_TO_COLLECT {
        let expected_value = U256::from(i as u64);
        let mut found = false;
        for event in collected_logs.iter() {
            if event.data.value == expected_value {
                found = true;
                break;
            }
        }
        assert!(found, "did not find {} in contract_2 logs", expected_value);
        tracing::info!("found {expected_value} in contract_2 events!");
    }
}

#[tokio::test]
async fn unsubscribe_log_stream_wait() {
    unsubscribe_log_stream(true).await
}

#[tokio::test]
async fn unsubscribe_log_stream_nowait() {
    unsubscribe_log_stream(false).await
}

async fn unsubscribe_log_stream(wait_for_subscriptions: bool) {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract = EventEmitterClient::new(&anvil, 0).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());

    controller.subscriptions.enable_logs(
        vec![*contract.address()],
        vec![
            EventEmitter::IntegerEvent::SIGNATURE_HASH,
            EventEmitter::StringEvent::SIGNATURE_HASH,
        ],
    );

    if wait_for_subscriptions {
        // wait for all subscriptions to be active
        wait_for_all_rpc_requests_landed(&controller).await;
    }

    controller
        .subscriptions
        .disable_logs(&[], &[EventEmitter::StringEvent::SIGNATURE_HASH]);

    if wait_for_subscriptions {
        wait_for_all_rpc_requests_landed(&controller).await;
    }

    const LOGS_TO_COLLECT: usize = 6;

    let collected_logs = Arc::new(std::sync::Mutex::new(HashSet::new()));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream.next().await {
                    let mut lock = collected_logs.lock().unwrap();
                    match EventEmitter::IntegerEvent::decode_log(&log.inner) {
                        Ok(event) => {
                            tracing::info!("got integer: {}", event.data.value);
                            lock.insert(event.data.value);
                        }
                        Err(_) => {
                            // Not an integer event, try string event
                            match EventEmitter::StringEvent::decode_log(&log.inner) {
                                Ok(_) => {
                                    panic!("Should not have gotten a string event, we removed the subscription");
                                }
                                Err(e) => {
                                    panic!("Failed to decode log as either IntegerEvent or StringEvent: {e}");
                                }
                            }
                        }
                    }

                    if lock.len() >= LOGS_TO_COLLECT {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT as u64 {
        // these shouldn't land, so send them first for early error checking
        let _ = contract
            .emitString(format!("test_string_{}", value))
            .send()
            .await
            .unwrap();
    }

    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();

    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    for i in 0..LOGS_TO_COLLECT as u64 {
        assert!(
            collected_logs.contains(&U256::from(i)),
            "did not find {} in integer logs",
            i
        );

        tracing::info!("found {i} in integer events!")
    }
}

#[tokio::test]
async fn controller_drop() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut block_height_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    controller.subscriptions.toggle_block_height(true);

    let height_count = Arc::new(AtomicUsize::new(0));

    const BLOCKS_TO_COLLECT: usize = 5;
    let handle = tokio::spawn({
        let height_count = height_count.clone();
        let controller = Arc::new(std::sync::Mutex::new(Some(controller)));

        async move {
            timeout(Duration::from_secs(10), async move {
                while let Some(height) = block_height_stream.next().await {
                    let count = height_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    tracing::info!("got height: {height}, count: {count}");
                    if count + 1 >= BLOCKS_TO_COLLECT {
                        let controller = controller.lock().unwrap().take().unwrap();
                        std::mem::drop(controller);
                    }
                }
            })
            .await
            .unwrap();
        }
    });

    handle.await.unwrap();

    let height_count = height_count.load(std::sync::atomic::Ordering::SeqCst);

    assert!(
        // give a little wiggle room
        (BLOCKS_TO_COLLECT..BLOCKS_TO_COLLECT + 3).contains(&height_count),
        "got {} blocks, not enough to test",
        height_count
    );
}

#[tokio::test]
async fn all_log_stream() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract = EventEmitterClient::new(&anvil, 0).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());

    controller.subscriptions.enable_logs(vec![], vec![]);

    const LOGS_TO_COLLECT: usize = 5;
    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream.next().await {
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

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();

    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs, not enough to test",
        collected_logs.len()
    );

    // Extract the events
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
async fn unsubscribe_all_log_stream_explicit() {
    unsubscribe_all_log_stream(true).await
}

#[tokio::test]
async fn unsubscribe_all_log_stream_implicit() {
    unsubscribe_all_log_stream(false).await
}

async fn unsubscribe_all_log_stream(explicit: bool) {
    init_tracing_tests();

    let anvil = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller,
        mut log_stream,
        ..
    } = EvmTriggerStreams::new(vec![anvil.ws_endpoint()], None);

    let contract = EventEmitterClient::new(&anvil, 0).deploy().await;

    tracing::info!("Deployed contract at {}", contract.address());

    if explicit {
        controller.subscriptions.enable_logs(vec![], vec![]);
        controller.subscriptions.disable_all_logs();
    } else {
        controller
            .subscriptions
            .enable_logs(vec![*contract.address()], vec![]);
        controller
            .subscriptions
            .disable_logs(&[*contract.address()], &[]);
    }

    wait_for_all_rpc_requests_landed(&controller).await;

    let got_a_timeout = Arc::new(AtomicBool::new(false));
    let got_a_log = Arc::new(AtomicBool::new(false));

    // First spawn a task to collect the logs into the stream
    let handle = tokio::spawn({
        let got_a_timeout = got_a_timeout.clone();
        let got_a_log = got_a_log.clone();
        async move {
            // 2 seconds is enough to see if we get a log or not
            let resp = timeout(Duration::from_secs(2), async {
                if log_stream.next().await.is_some() {
                    got_a_log.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            })
            .await;

            if resp.is_err() {
                got_a_timeout.store(true, std::sync::atomic::Ordering::SeqCst);
            };
        }
    });

    // Now, since we're not blocked, we can emit the events
    for value in 0..10u64 {
        let _ = contract
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    if !got_a_timeout.load(std::sync::atomic::Ordering::SeqCst) {
        panic!("Never got the timeout");
    }

    if got_a_log.load(std::sync::atomic::Ordering::SeqCst) {
        panic!("Got a log, even though we unsubscribed from all logs");
    }
}

#[tokio::test]
async fn multiple_clients() {
    init_tracing_tests();

    let anvil_1 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let anvil_2 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller: controller_1,
        log_stream: mut log_stream_1,
        ..
    } = EvmTriggerStreams::new(vec![anvil_1.ws_endpoint()], None);

    let EvmTriggerStreams {
        controller: controller_2,
        log_stream: mut log_stream_2,
        ..
    } = EvmTriggerStreams::new(vec![anvil_2.ws_endpoint()], None);

    let contract_1 = EventEmitterClient::new(&anvil_1, 0).deploy().await;
    tracing::info!("Deployed contract_1 at {}", contract_1.address());

    let contract_2 = EventEmitterClient::new(&anvil_2, 0).deploy().await;
    tracing::info!("Deployed contract_2 at {}", contract_2.address());

    controller_1.subscriptions.enable_logs(
        vec![*contract_1.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    controller_2.subscriptions.enable_logs(
        vec![*contract_2.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    wait_for_all_rpc_requests_landed(&controller_1).await;
    wait_for_all_rpc_requests_landed(&controller_2).await;

    const LOGS_TO_COLLECT: usize = 5;

    let collected_logs_1 = Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected_logs_2 = Arc::new(std::sync::Mutex::new(Vec::new()));

    // First spawn a task to collect the logs into the stream

    let handle_1 = tokio::spawn({
        let collected_logs_1 = collected_logs_1.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream_1.next().await {
                    let mut lock = collected_logs_1.lock().unwrap();
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

    let handle_2 = tokio::spawn({
        let collected_logs_2 = collected_logs_2.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream_2.next().await {
                    let mut lock = collected_logs_2.lock().unwrap();
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

    // Now, since we're not blocked, we can emit the events

    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract_1
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();

        let _ = contract_2
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle_1.await.unwrap();
    handle_2.await.unwrap();

    let collected_logs_1 = collected_logs_1.lock().unwrap();
    let collected_logs_2 = collected_logs_2.lock().unwrap();

    assert!(
        collected_logs_1.len() >= LOGS_TO_COLLECT,
        "only got {} logs from client 1, not enough to test",
        collected_logs_1.len()
    );

    assert!(
        collected_logs_2.len() >= LOGS_TO_COLLECT,
        "only got {} logs from client 2, not enough to test",
        collected_logs_2.len()
    );

    // Extract the events
    let mut collected_values_1 = HashSet::new();
    for log in collected_logs_1.iter() {
        let event = EventEmitter::IntegerEvent::decode_log(&log.inner)
            .unwrap()
            .data;

        collected_values_1.insert(event.value);
    }
    let mut collected_values_2 = HashSet::new();
    for log in collected_logs_2.iter() {
        let event = EventEmitter::IntegerEvent::decode_log(&log.inner)
            .unwrap()
            .data;

        collected_values_2.insert(event.value);
    }

    for value in 0..LOGS_TO_COLLECT as u64 {
        assert!(
            collected_values_1.contains(&U256::from(value)),
            "did not find emitted value {} in logs from client 1",
            value
        );

        tracing::info!("found {value} in events from client 1!");

        assert!(
            collected_values_2.contains(&U256::from(value)),
            "did not find emitted value {} in logs from client 2",
            value
        );

        tracing::info!("found {value} in events from client 2!");
    }
}

#[tokio::test]
async fn multiple_clients_drop_one() {
    init_tracing_tests();

    let anvil_1 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));
    let anvil_2 = safe_spawn_anvil_extra(|anvil| anvil.block_time_f64(0.02));

    let EvmTriggerStreams {
        controller: controller_1,
        ..
    } = EvmTriggerStreams::new(vec![anvil_1.ws_endpoint()], None);

    let EvmTriggerStreams {
        controller: controller_2,
        log_stream: mut log_stream_2,
        ..
    } = EvmTriggerStreams::new(vec![anvil_2.ws_endpoint()], None);

    let contract_1 = EventEmitterClient::new(&anvil_1, 0).deploy().await;
    tracing::info!("Deployed contract_1 at {}", contract_1.address());

    let contract_2 = EventEmitterClient::new(&anvil_2, 0).deploy().await;
    tracing::info!("Deployed contract_2 at {}", contract_2.address());

    controller_1.subscriptions.enable_logs(
        vec![*contract_1.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    controller_2.subscriptions.enable_logs(
        vec![*contract_2.address()],
        vec![EventEmitter::IntegerEvent::SIGNATURE_HASH],
    );

    drop(controller_1);

    wait_for_all_rpc_requests_landed(&controller_2).await;

    const LOGS_TO_COLLECT: usize = 5;

    let collected_logs = Arc::new(std::sync::Mutex::new(Vec::new()));

    // First spawn a task to collect the logs into the stream

    let handle = tokio::spawn({
        let collected_logs = collected_logs.clone();
        async move {
            timeout(Duration::from_secs(5), async {
                while let Some(log) = log_stream_2.next().await {
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

    // Now, since we're not blocked, we can emit the events
    for value in 0..LOGS_TO_COLLECT as u64 {
        let _ = contract_2
            .emitInteger(U256::from(value))
            .send()
            .await
            .unwrap();
    }

    // but we do need to wait for the handle to finish
    handle.await.unwrap();

    let collected_logs = collected_logs.lock().unwrap();
    assert!(
        collected_logs.len() >= LOGS_TO_COLLECT,
        "only got {} logs from client 2, not enough to test",
        collected_logs.len()
    );

    // Extract the events
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
            "did not find emitted value {} in logs from client 2",
            value
        );

        tracing::info!("found {value} in events from client 2!");
    }
}
