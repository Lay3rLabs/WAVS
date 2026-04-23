mod helpers;

use crate::helpers::exec::try_execute_component_raw;
use utils::{
    init_tracing_tests,
    storage::db::WavsDb,
    test_utils::mock_engine::COMPONENT_MULTI_STEP_AGENT_BYTES,
};
use wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx;
use wasmtime::{Config as WTConfig, Engine as WTEngine};

/// Execute the multi-step-agent WASM and return the raw response payload.
///
/// Uses `try_execute_component_raw` which calls `execute()` (the engine entry point).
/// The engine detects the `wavs:operator/agent` export, enters the continuation loop,
/// and returns only after `StepResult::Done`.
async fn run_multi_step_agent(kv_ctx: KeyValueCtx) -> Result<Vec<u8>, String> {
    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let mut payloads = try_execute_component_raw(
        engine,
        COMPONENT_MULTI_STEP_AGENT_BYTES,
        Default::default(),
        Some(kv_ctx),
        // The multi-step-agent ignores trigger data; pass empty bytes
        vec![],
    )
    .await?;

    payloads
        .pop()
        .ok_or_else(|| "agent produced no output".to_string())
}

/// The agent must run exactly 4 invocations (steps 0, 1, 2 → Continue; step 3 → Done)
/// and return a JSON array of checkpoint messages in the final payload.
#[tokio::test]
async fn multi_step_agent_runs_to_completion() {
    init_tracing_tests();

    let db = WavsDb::new().unwrap();
    let kv_ctx = KeyValueCtx::new(db.clone(), "test-svc".to_string());

    let payload = run_multi_step_agent(kv_ctx)
        .await
        .expect("agent should complete without error");

    // The agent returns a JSON array: ["checkpoint:0: completed step 0", ...]
    let summary: Vec<String> = serde_json::from_slice(&payload)
        .expect("agent payload should be valid JSON array of strings");

    assert_eq!(
        summary.len(),
        4,
        "Expected 4 checkpoint messages (steps 0-3), got {}",
        summary.len()
    );

    assert!(
        summary[0].contains("checkpoint:0"),
        "First entry should contain checkpoint:0"
    );
    assert!(
        summary[3].contains("checkpoint:3"),
        "Last entry should contain checkpoint:3"
    );
}

/// After the agent completes, the KV store should contain observable checkpoint entries
/// in the `agent_state` bucket under the `test-svc` namespace.
///
/// Key format: `{namespace}/{bucket_id}/{key}`
/// → `test-svc/agent_state/checkpoint:0`, `test-svc/agent_state/checkpoint:1`, etc.
#[tokio::test]
async fn multi_step_agent_kv_checkpoints_exist() {
    init_tracing_tests();

    let db = WavsDb::new().unwrap();
    let kv_ctx = KeyValueCtx::new(db.clone(), "test-svc".to_string());

    run_multi_step_agent(kv_ctx)
        .await
        .expect("agent should complete without error");

    // Verify component-written checkpoints exist at known keys
    for step in 0..4u32 {
        let key = format!("test-svc/agent_state/checkpoint:{step}");
        let value = db
            .kv_store
            .get_cloned(&key)
            .unwrap_or_else(|| panic!("checkpoint:{step} not found in KV (key: {key})"));

        let msg = String::from_utf8(value).expect("checkpoint value should be valid UTF-8");
        assert_eq!(
            msg,
            format!("completed step {step}"),
            "checkpoint:{step} has unexpected value"
        );
    }

    // Verify the step counter was updated
    let counter_key = "test-svc/agent_state/step_counter";
    let counter_bytes = db
        .kv_store
        .get_cloned(&counter_key.to_string())
        .expect("step_counter should exist in KV");
    let counter: u32 = String::from_utf8(counter_bytes)
        .expect("counter should be UTF-8")
        .parse()
        .expect("counter should be a number");
    assert_eq!(counter, 4, "step_counter should be 4 after completing all steps");
}
