use thiserror::Error;
use wavs_types::{ServiceID, WorkflowID};

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
        service_id: ServiceID,
        workflow_id: WorkflowID,
    },

    #[error{"Unable to get component input: {0}"}]
    Input(anyhow::Error),

    #[error{"Filesystem: {0}"}]
    Filesystem(anyhow::Error),

    #[error{"Unable to set store: {0}"}]
    Store(anyhow::Error),

    #[error("Max fuel consumed by WasmEngine for service: {0}, workflow: {1}")]
    OutOfFuel(ServiceID, WorkflowID),

    #[error("Time limit exceeded by WasmEngine for service: {0}, workflow: {1}")]
    OutOfTime(ServiceID, WorkflowID),
}
