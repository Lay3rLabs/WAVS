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

async fn do_it(kind: &str) {
    init_tracing_tests();

    let mut wt_config = WTConfig::new();

    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let engine_ticker = engine.weak();
    // just run forever, ticking forward till the end of time (or however long this node is up)
    std::thread::spawn(move || loop {
        if let Some(engine) = engine_ticker.upgrade() {
            engine.increment_epoch();
        } else {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    });

    let kind = kind.to_string();

    // try to tie up the runtime
    tokio::spawn(execute_component_raw(
        engine.clone(),
        COMPONENT_ECHO_DATA_BYTES,
        [
            ("sleep-ms".to_string(), "10000".to_string()),
            ("sleep-kind".to_string(), kind.clone()),
        ]
        .into_iter()
        .collect(),
        None,
        b"long".to_vec(),
    ));

    let short_fut = tokio::spawn(execute_component_raw(
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
    ));

    let time = std::time::Instant::now();
    loop {
        if short_fut.is_finished() {
            break;
        }

        tokio::task::yield_now().await;
    }

    if time.elapsed() >= Duration::from_secs(5) {
        panic!("took way too long for tasks to complete!");
    }
}
