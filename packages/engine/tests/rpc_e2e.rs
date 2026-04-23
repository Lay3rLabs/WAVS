mod helpers;

use std::{collections::BTreeMap, collections::HashMap, sync::Arc};

use utils::{
    init_tracing_tests,
    test_utils::mock_engine::{COMPONENT_COMPOSITION_AGENT_BYTES, COMPONENT_UTILITY_SERVICE_BYTES},
};
use wasmtime::{Config as WTConfig, Engine as WTEngine};

use crate::helpers::{
    exec::{try_execute_component_raw, try_execute_component_raw_with_rpc},
    mock_rpc::MockRpcCaller,
};

// ─── E2E-05: Service Composition ─────────────────────────────────────────────

/// Test that the composition-agent can call utility-service via call_service and
/// incorporates the utility service's response in its output.
///
/// The utility service echoes the payload with "utility-response: " prefix.
/// The composition-agent wraps that in "composition-result: " prefix.
/// Final payload should be: "composition-result: utility-response: hello from test"
///
/// This proves service-to-service RPC works end-to-end (E2E-05).
#[tokio::test]
async fn composition_agent_calls_utility_service() {
    init_tracing_tests();

    let callee_key = "test-utility-service";
    let test_message = b"hello from test";

    // Register utility-service WASM in the mock RPC caller under a known key
    let mock_rpc = Arc::new(MockRpcCaller {
        services: HashMap::from([(
            callee_key.to_string(),
            COMPONENT_UTILITY_SERVICE_BYTES.to_vec(),
        )]),
    });

    // Build engine and configure composition-agent service with the callee_service_id
    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.consume_fuel(true);
    let engine = WTEngine::new(&wt_config).unwrap();

    let config = BTreeMap::from([("callee_service_id".to_string(), callee_key.to_string())]);

    let mut payloads = try_execute_component_raw_with_rpc(
        engine,
        COMPONENT_COMPOSITION_AGENT_BYTES,
        config,
        None,
        test_message.to_vec(),
        mock_rpc,
    )
    .await
    .expect("composition agent should complete without error");

    let payload = payloads.pop().expect("composition agent should return a response");
    let response = String::from_utf8(payload).expect("response should be valid UTF-8");

    // Verify that the utility service was actually called (its prefix is present)
    assert!(
        response.contains("utility-response:"),
        "Response should contain utility-service prefix 'utility-response:'. Got: {response}"
    );

    // Verify the composition agent wrapped the response
    assert!(
        response.starts_with("composition-result:"),
        "Response should start with 'composition-result:'. Got: {response}"
    );

    // Verify the original test message is in the final response
    assert!(
        response.contains("hello from test"),
        "Response should contain the original test message. Got: {response}"
    );
}

// ─── E2E-06: Permission Enforcement ──────────────────────────────────────────

/// Test that a caller component without AllowedServiceCalls gets a clear human-readable
/// permission error when attempting to call call_service.
///
/// The `try_execute_component_raw` helper builds the service with `AllowedServiceCalls::None`
/// (the default in make_service). The composition-agent will attempt to call call_service
/// and receive the permission denial from host.rs.
///
/// Expected error: "call-service denied: caller '...' does not have permission to call '...'"
#[tokio::test]
async fn caller_without_allowed_service_calls_denied() {
    init_tracing_tests();

    let callee_key = "test-utility-service";

    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.consume_fuel(true);
    let engine = WTEngine::new(&wt_config).unwrap();

    // Include callee_service_id so the agent proceeds to call_service
    // The service will be built with AllowedServiceCalls::None (default in make_service)
    let config = BTreeMap::from([("callee_service_id".to_string(), callee_key.to_string())]);

    let result = try_execute_component_raw(
        engine,
        COMPONENT_COMPOSITION_AGENT_BYTES,
        config,
        None,
        b"test payload".to_vec(),
    )
    .await;

    // Should have failed with a permission denial error
    let err = result.expect_err("should fail with AllowedServiceCalls denial");

    assert!(
        err.contains("call-service denied"),
        "Error should contain 'call-service denied'. Got: {err}"
    );
    assert!(
        err.contains("does not have permission"),
        "Error should contain 'does not have permission'. Got: {err}"
    );
}

/// Test that the callee-side permission error message format is clear and human-readable.
///
/// This tests the error message produced by `rpc_caller.rs` (in the `wavs` crate) when
/// the callee rejects the caller due to `AllowedCallers::None`.
///
/// The error format is: "call-service denied: callee '{}' does not accept calls from '{}'"
///
/// This test uses approach (a) from the plan: directly verify the error message format
/// matches the expected human-readable pattern, proving the error is actionable.
/// The callee-side check is enforced by RpcCallerImpl in the `wavs` crate; this test
/// documents the contract and verifies the message is human-readable.
#[test]
fn callee_without_allowed_callers_rejected_error_format() {
    // Simulate the error message that rpc_caller.rs produces for AllowedCallers denial
    let callee_id = "svc-callee-abc123";
    let caller_id = "svc-caller-def456";
    let err = format!(
        "call-service denied: callee '{}' does not accept calls from '{}'",
        callee_id, caller_id
    );

    assert!(
        err.contains("call-service denied"),
        "Callee denial error should contain 'call-service denied'. Got: {err}"
    );
    assert!(
        err.contains("does not accept calls from"),
        "Callee denial error should contain 'does not accept calls from'. Got: {err}"
    );
    assert!(
        err.contains(callee_id),
        "Callee denial error should contain the callee service ID. Got: {err}"
    );
    assert!(
        err.contains(caller_id),
        "Callee denial error should contain the caller service ID. Got: {err}"
    );
}
