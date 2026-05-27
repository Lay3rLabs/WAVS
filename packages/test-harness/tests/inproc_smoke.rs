//! In-process runner smoke test using a bundled example component.
//!
//! Runs `examples/build/components/echo_data.wasm` once via [`InProcRunner`].
//! Skips gracefully if the example artifact is missing (e.g. on a fresh
//! checkout that hasn't run `just wasi-build-native` yet).

use std::path::PathBuf;

use wavs_test_harness::service::{InProcRunner, ServiceSpec};

fn example_wasm(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/build/components")
        .join(name);
    p.canonicalize().unwrap_or(p)
}

#[tokio::test]
async fn echo_data_round_trip() {
    let component = example_wasm("echo_data.wasm");
    let aggregator = example_wasm("simple_aggregator.wasm");

    if !component.exists() {
        eprintln!(
            "[skipping] {} not found — run `just wasi-build-native` to build example components",
            component.display()
        );
        return;
    }
    if !aggregator.exists() {
        eprintln!(
            "[skipping] {} not found — run `just wasi-build-native` to build example components",
            aggregator.display()
        );
        return;
    }

    let spec = ServiceSpec::new()
        .component_wasm(&component)
        .aggregator_wasm(&aggregator)
        .operator_count(1);

    let runner = InProcRunner::from_spec(&spec).expect("build runner");

    // echo_data wraps its input in a DataWithId envelope; for the harness we
    // only care that the component executed and produced *some* payload.
    let input = wavs_test_harness::lifecycle::manual_input_json(&serde_json::json!({
        "id": 1,
        "data": "hello-from-harness"
    }))
    .unwrap();

    let outputs = runner.run_component(input).await.expect("run component");
    assert!(
        !outputs.is_empty(),
        "echo_data should emit at least one payload"
    );
}
