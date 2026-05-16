//! Minimal local example — spawn Anvil, run an example component once.
//!
//! Run with: `cargo run -p wavs-test-harness --example minimal_local`
//! Prerequisite: `just wasi-build-native echo_data simple_aggregator`

use std::path::PathBuf;

use wavs_test_harness::{
    chain,
    service::{InProcRunner, ServiceSpec},
    TestHarness,
};

fn example_wasm(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/build/components")
        .join(name)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let component = example_wasm("echo_data.wasm");
    let aggregator = example_wasm("simple_aggregator.wasm");
    if !component.exists() || !aggregator.exists() {
        eprintln!(
            "Missing example WASM. Run `just wasi-build-native` from the WAVS repo root."
        );
        return Ok(());
    }

    let spec = ServiceSpec::new()
        .component_wasm(&component)
        .aggregator_wasm(&aggregator)
        .config_var("DEMO_KEY", "demo-value")
        .operator_count(1);

    let (provider, anvil) = chain::spawn_local().await?;
    let harness = TestHarness::new(provider, anvil, &spec)?;
    println!(
        "[example] Anvil at endpoint {}",
        chain::redact_url(&harness.anvil.endpoint())
    );

    // Drive a single component execution.
    let input = wavs_test_harness::lifecycle::manual_input_json(&serde_json::json!({
        "id": 1,
        "data": "hello-from-example"
    }))?;
    let outputs = harness.runner.run_component(input).await?;
    println!(
        "[example] component emitted {} payload(s), first {} bytes",
        outputs.len(),
        outputs.first().map(|v| v.len()).unwrap_or(0)
    );

    // Block-time control demo.
    harness.mine_blocks(3).await?;

    // Snapshot / revert demo.
    let snap = harness.snapshot().await?;
    harness.mine_blocks(5).await?;
    snap.revert(&harness.provider).await?;
    println!("[example] snapshot revert verified");

    // Belt-and-suspenders: keep clippy happy about unused runner field.
    drop(InProcRunner::from_spec(&spec)?);
    Ok(())
}
