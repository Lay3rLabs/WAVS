use thiserror::Error;
use wavs_types::{ComponentDigest, ServiceId, WorkflowId};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Wasm instantiate: {0}")]
    Instantiate(anyhow::Error),

    #[error("Wasm exec result: {0}")]
    ExecResult(String),

    #[error("Component returned an error: {0}")]
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

    #[error("Unable to add to linker: {0}")]
    AddToLinker(wasmtime::Error),

    #[error("Compile error: {0}")]
    Compile(anyhow::Error),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("IO error: {0}")]
    IO(String),

    #[error("Unknown digest: {0}")]
    UnknownDigest(ComponentDigest),

    #[error("Registry: {0}")]
    Registry(#[from] wasm_pkg_client::Error),
}
