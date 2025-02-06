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

    #[error{"unable to get trigger data as component input: {0}"}]
    TriggerData(anyhow::Error),

    #[error{"filesystem: {0}"}]
    Filesystem(anyhow::Error),

    #[error{"unable to set store: {0}"}]
    Store(anyhow::Error),

    #[error("Max fuel consumed by WasmEngine for service: {0}, workflow: {1}")]
    OutOfFuel(ServiceID, WorkflowID),
}
