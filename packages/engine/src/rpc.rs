use std::{future::Future, pin::Pin};

pub type RpcResult = Result<Vec<u8>, String>;
pub type RpcFuture<'a> = Pin<Box<dyn Future<Output = RpcResult> + Send + 'a>>;

/// Injected into OperatorHostComponent so call_service can execute callee components
/// without creating a circular dependency on the `wavs` crate.
pub trait RpcCaller: Send + Sync {
    /// Execute a callee service and return the first response payload.
    /// `caller_id` is the calling service's ID string.
    /// `call_stack` tracks the in-flight call chain for cycle detection.
    fn call(
        &self,
        callee_id: String,
        payload: Vec<u8>,
        call_stack: Vec<String>,
    ) -> RpcFuture<'_>;
}
