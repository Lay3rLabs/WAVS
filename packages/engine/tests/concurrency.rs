mod helpers;

use std::time::Duration;

use crate::helpers::exec::execute_component_raw;
use utils::{init_tracing_tests, test_utils::mock_engine::COMPONENT_ECHO_DATA_BYTES};
use wasmtime::{Config as WTConfig, Engine as WTEngine};

#[tokio::test(flavor = "current_thread")]
async fn concurrency_async() {
    do_it("async").await;
}

#[tokio::test(flavor = "current_thread")]
async fn concurrency_sync() {
    do_it("sync").await;
}

#[tokio::test(flavor = "current_thread")]
async fn concurrency_hotloop() {
    do_it("hotloop").await;
}

async fn do_it(kind: impl ToString) {
    init_tracing_tests();

    let mut wt_config = WTConfig::new();

    wt_config.wasm_component_model(true);
    wt_config.epoch_interruption(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();
    let kind = kind.to_string();

    let time = std::time::Instant::now();

    // just run forever, ticking forward till the end of time (or however long this node is up)
    let engine_ticker = engine.weak();
    std::thread::spawn(move || loop {
        if let Some(engine_ticker) = engine_ticker.upgrade() {
            engine_ticker.increment_epoch();
        } else {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    });

    // try to tie up the runtime
    let (slow_tx, _) = crossbeam::channel::unbounded();
    tokio::spawn({
        let engine = engine.clone();
        let kind = kind.clone();
        async move {
            let res = execute_component_raw(
                engine,
                COMPONENT_ECHO_DATA_BYTES,
                [
                    ("sleep-ms".to_string(), "10000".to_string()),
                    ("sleep-kind".to_string(), kind),
                ]
                .into_iter()
                .collect(),
                None,
                b"long".to_vec(),
            )
            .await;

            slow_tx.send(res).unwrap();
        }
    });

    // so that this quick task doesn't complete fast
    let (quick_tx, quick_rx) = crossbeam::channel::unbounded();
    tokio::spawn({
        async move {
            let res = execute_component_raw(
                engine,
                COMPONENT_ECHO_DATA_BYTES,
                [
                    ("sleep-ms".to_string(), "10".to_string()),
                    ("sleep-kind".to_string(), kind),
                ]
                .into_iter()
                .collect(),
                None,
                b"short".to_vec(),
            )
            .await;

            quick_tx.send(res).unwrap();
        }
    });

    let res = tokio::task::spawn_blocking(move || quick_rx.recv().unwrap())
        .await
        .unwrap();

    assert_eq!(res[0], b"short".to_vec());

    if time.elapsed() >= Duration::from_secs(10) {
        panic!(
            "took way too long for tasks to complete! ({}ms)",
            time.elapsed().as_millis()
        );
    }

    println!("Success! took {}ms", time.elapsed().as_millis());
}
