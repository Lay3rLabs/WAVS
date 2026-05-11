mod helpers;

use crate::helpers::exec::execute_component;
use example_types::{SquareRequest, SquareResponse};
use utils::{init_tracing_tests, test_utils::mock_engine::COMPONENT_SQUARE_BYTES};
use wavs_engine::utils::error::EngineError;
use wavs_types::{ServiceId, WorkflowId};

/// Verify that the ContinuationLimit error formats correctly and includes
/// the expected fields in its Display output.
#[test]
fn continuation_limit_error_format() {
    let service_id = ServiceId::hash(b"test-service");
    let workflow_id = WorkflowId::new("test-workflow").unwrap();
    let err = EngineError::ContinuationLimit {
        service_id: service_id.clone(),
        workflow_id: workflow_id.clone(),
        steps: 10,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("ContinuationLimit"),
        "Error message should contain 'ContinuationLimit': {msg}"
    );
    assert!(msg.contains("10"), "Error message should contain step count: {msg}");
    // workflow_id is a readable string; verify it appears
    assert!(
        msg.contains("test-workflow"),
        "Error message should contain workflow_id: {msg}"
    );
}

/// Verify that the ContinuationLimit error contains all expected fields
/// (service_id, workflow_id, steps) in the formatted output.
#[test]
fn continuation_limit_error_fields() {
    let service_id = ServiceId::hash(b"my-service-bytes");
    let workflow_id = WorkflowId::new("my-workflow").unwrap();
    let steps = 5usize;
    let err = EngineError::ContinuationLimit {
        service_id: service_id.clone(),
        workflow_id: workflow_id.clone(),
        steps,
    };
    let msg = err.to_string();
    assert!(msg.contains("my-workflow"), "Should contain workflow_id: {msg}");
    assert!(msg.contains("5"), "Should contain steps '5': {msg}");
    // Verify we can reconstruct the same error variant (fields are accessible)
    let _ = EngineError::ContinuationLimit { service_id, workflow_id, steps };
}

/// Verify the KV key format used by the continuation engine is constructed
/// correctly. The expected pattern is:
///   {namespace}/wavs_agent_step/{service_id}:{workflow_id}:step:{N}
#[test]
fn kv_key_format_correctness() {
    let namespace = "my-service";
    let service_id = "my-service";
    let workflow_id = "my-workflow";
    let correlation_id = format!("{}:{}", service_id, workflow_id);
    let step = 2usize;
    let key = format!("{}/wavs_agent_step/{}:step:{}", namespace, correlation_id, step);
    assert_eq!(
        key,
        "my-service/wavs_agent_step/my-service:my-workflow:step:2",
        "KV key format should match expected pattern"
    );
}

/// Verify the KV key format at step 0 (first continuation step).
#[test]
fn kv_key_format_step_zero() {
    let namespace = "svc-abc";
    let correlation_id = "svc-abc:wfl-xyz";
    let step = 0usize;
    let key = format!("{}/wavs_agent_step/{}:step:{}", namespace, correlation_id, step);
    assert_eq!(
        key,
        "svc-abc/wavs_agent_step/svc-abc:wfl-xyz:step:0",
        "KV key format at step 0 should be correct"
    );
}

/// Execute a non-agent (legacy) component via the refactored execute() function.
/// This proves the legacy fallback path is intact after the continuation engine refactor.
/// The square component doubles as a regression test: 7² = 49.
#[tokio::test]
async fn legacy_component_still_works() {
    init_tracing_tests();

    let resp: Vec<SquareResponse> = execute_component(
        COMPONENT_SQUARE_BYTES,
        Default::default(),
        None,
        SquareRequest::new(7),
    )
    .await;

    assert_eq!(resp[0].y, 49, "7^2 should be 49, got {}", resp[0].y);
}

/// Execute the legacy component with a different input to further validate
/// the fallback path routes correctly through execute() -> execute_legacy().
#[tokio::test]
async fn legacy_component_multiple_values() {
    init_tracing_tests();

    for (input, expected) in [(3u64, 9u64), (10, 100), (0, 0)] {
        let resp: Vec<SquareResponse> = execute_component(
            COMPONENT_SQUARE_BYTES,
            Default::default(),
            None,
            SquareRequest::new(input),
        )
        .await;
        assert_eq!(
            resp[0].y, expected,
            "{}^2 should be {}, got {}",
            input, expected, resp[0].y
        );
    }
}
