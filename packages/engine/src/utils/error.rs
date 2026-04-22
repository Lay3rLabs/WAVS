use thiserror::Error;
use wavs_types::{ComponentDigest, ServiceId, WorkflowId};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Wasm instantiate: {0}")]
    Instantiate(anyhow::Error),

    #[error("Wasm exec result: {0}")]
    ExecResult(String),

    #[error("Component returned an error: {0:?}")]
    ComponentError(anyhow::Error),

    #[error("Workflow {workflow_id} not found for service {service_id}")]
    WorkflowNotFound {
        service_id: ServiceId,
        workflow_id: WorkflowId,
    },

    #[error{"Unable to get component input: {0}"}]
    Input(anyhow::Error),

    #[error{"Filesystem: {0}"}]
    Filesystem(anyhow::Error),

    #[error{"Unable to set store: {0}"}]
    Store(anyhow::Error),

    #[error("Max fuel consumed by WasmEngine for service: {0}, workflow: {1}")]
    OutOfFuel(ServiceId, WorkflowId),

    #[error("Time limit exceeded by WasmEngine for service: {0}, workflow: {1}")]
    OutOfTime(ServiceId, WorkflowId),

    #[error("ContinuationLimit: exceeded {steps} steps for service: {service_id}, workflow: {workflow_id}")]
    ContinuationLimit {
        service_id: ServiceId,
        workflow_id: WorkflowId,
        steps: usize,
    },

    #[error("Unable to add to linker: {0}")]
    AddToLinker(wasmtime::Error),

    #[error("Compile error: {0}")]
    Compile(anyhow::Error),

    #[error("Wasm response is malformed: {0}")]
    WasmResponseMalformed(anyhow::Error),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("IO error: {0}")]
    IO(String),

    #[error("Unknown digest: {0}")]
    UnknownDigest(ComponentDigest),

    #[error("Registry: {0}")]
    Registry(#[from] wasm_pkg_client::Error),

    #[error("When returning multiple responses, they must all have an event id salt")]
    MissingEventIdSalt,

    #[error("Wasm response size limit exceeded: {0}")]
    ResponseSizeExceeded(#[from] wavs_types::WasmResponseSizeError),

    #[error("Mismatched instance data and logger. Data: {data}, Logger: {logger}")]
    MismatchedInstanceDataAndLogger {
        data: &'static str,
        logger: &'static str,
    },

    #[error("call-service permission denied: caller '{caller_id}' cannot call '{callee_id}': {reason}")]
    RpcPermissionDenied {
        caller_id: String,
        callee_id: String,
        reason: String,
    },

    #[error("call-service cycle detected: '{callee_id}' already in call chain {call_chain:?}")]
    RpcCycleDetected {
        callee_id: String,
        call_chain: Vec<String>,
    },

    #[error("call-service depth limit ({limit}) exceeded: call chain {call_chain:?}")]
    RpcDepthExceeded {
        limit: usize,
        call_chain: Vec<String>,
    },
}
