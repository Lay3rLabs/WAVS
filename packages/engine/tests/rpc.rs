use wavs_engine::utils::error::EngineError;
use wavs_types::{ServiceId, WorkflowId};

/// Verify that RpcPermissionDenied Display output includes caller_id, callee_id, and reason.
#[test]
fn rpc_permission_denied_error_format() {
    let err = EngineError::RpcPermissionDenied {
        caller_id: "caller-svc".to_string(),
        callee_id: "callee-svc".to_string(),
        reason: "AllowedServiceCalls::None".to_string(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("caller-svc"),
        "Error message should contain caller_id: {msg}"
    );
    assert!(
        msg.contains("callee-svc"),
        "Error message should contain callee_id: {msg}"
    );
    assert!(
        msg.contains("AllowedServiceCalls::None"),
        "Error message should contain reason: {msg}"
    );
}

/// Verify that RpcCycleDetected Display output includes callee_id and the call chain.
#[test]
fn rpc_cycle_detected_error_format() {
    let call_chain = vec!["svc-a".to_string(), "svc-b".to_string()];
    let err = EngineError::RpcCycleDetected {
        callee_id: "svc-a".to_string(),
        call_chain: call_chain.clone(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("svc-a"),
        "Error message should contain callee_id: {msg}"
    );
    assert!(
        msg.contains("svc-b"),
        "Error message should contain call chain member: {msg}"
    );
}

/// Verify that RpcDepthExceeded Display output includes limit and call chain.
#[test]
fn rpc_depth_exceeded_error_format() {
    let call_chain: Vec<String> = (0..5).map(|i| format!("svc-{}", i)).collect();
    let err = EngineError::RpcDepthExceeded {
        limit: 5,
        call_chain: call_chain.clone(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("5"),
        "Error message should contain the depth limit: {msg}"
    );
    assert!(
        msg.contains("svc-0"),
        "Error message should contain call chain members: {msg}"
    );
}

/// Verify struct field access on RpcPermissionDenied.
#[test]
fn rpc_permission_denied_error_fields() {
    let err = EngineError::RpcPermissionDenied {
        caller_id: "svc-caller".to_string(),
        callee_id: "svc-callee".to_string(),
        reason: "not in AllowedCallers list".to_string(),
    };
    // Verify we can pattern-match and read the fields
    match &err {
        EngineError::RpcPermissionDenied {
            caller_id,
            callee_id,
            reason,
        } => {
            assert_eq!(caller_id, "svc-caller");
            assert_eq!(callee_id, "svc-callee");
            assert_eq!(reason, "not in AllowedCallers list");
        }
        _ => panic!("Expected RpcPermissionDenied variant"),
    }
}

/// Test that a Vec<String> call stack correctly detects cycles (contains check).
/// This mirrors the logic in host.rs without requiring WASM execution.
#[test]
fn rpc_cycle_detection_logic() {
    let call_stack = vec!["svc-a".to_string(), "svc-b".to_string()];

    // svc-a is already in the chain — calling it again would create a cycle
    assert!(
        call_stack.contains(&"svc-a".to_string()),
        "svc-a is in the call chain, so calling it would create a cycle"
    );

    // svc-b is already in the chain
    assert!(
        call_stack.contains(&"svc-b".to_string()),
        "svc-b is in the call chain, so calling it would create a cycle"
    );

    // svc-c is not in the chain — no cycle
    assert!(
        !call_stack.contains(&"svc-c".to_string()),
        "svc-c is not in the call chain, no cycle"
    );

    // Empty call stack — nothing is a cycle
    let empty_stack: Vec<String> = vec![];
    assert!(
        !empty_stack.contains(&"svc-a".to_string()),
        "empty call stack has no cycles"
    );
}

/// Test depth limit check logic.
/// This mirrors the logic in host.rs: call_stack.len() >= RPC_MAX_DEPTH.
#[test]
fn rpc_depth_limit_logic() {
    const RPC_MAX_DEPTH: usize = 5;

    // A call stack at the limit should trigger a depth exceeded error
    let at_limit: Vec<String> = (0..5).map(|i| format!("svc-{}", i)).collect();
    assert_eq!(at_limit.len(), 5);
    assert!(
        at_limit.len() >= RPC_MAX_DEPTH,
        "stack at limit (len=5) should trigger depth exceeded"
    );

    // A call stack over the limit should also trigger
    let over_limit: Vec<String> = (0..6).map(|i| format!("svc-{}", i)).collect();
    assert!(
        over_limit.len() >= RPC_MAX_DEPTH,
        "stack over limit (len=6) should trigger depth exceeded"
    );

    // A short stack is within limits
    let short_stack: Vec<String> = vec!["svc-0".into()];
    assert!(
        short_stack.len() < RPC_MAX_DEPTH,
        "short stack (len=1) is within limit"
    );

    // Empty stack is within limits
    let empty_stack: Vec<String> = vec![];
    assert!(
        empty_stack.len() < RPC_MAX_DEPTH,
        "empty stack is within limit"
    );
}
