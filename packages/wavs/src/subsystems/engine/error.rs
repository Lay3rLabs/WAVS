use thiserror::Error;
use utils::storage::CAStorageError;
use wavs_types::{ComponentDigest, ServiceId, WorkflowId};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error{"Compile: {0}"}]
    Compile(anyhow::Error),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ServiceId, WorkflowId),

    #[error("No wasm found for digest {0}")]
    UnknownDigest(ComponentDigest),

    #[error{"{0}"}]
    Engine(#[from] wavs_engine::utils::error::EngineError),

    #[error{"Unable to send result after executing Service {0} / Workflow {1}"}]
    WasiResultSend(ServiceId, WorkflowId),

    #[error("No registry configured")]
    NoRegistry,

    #[error("could not encode EventId {0:?}")]
    EncodeEventId(bincode::error::EncodeError),

    #[error("could not encode EventOrder {0:?}")]
    EncodeEventOrder(bincode::error::EncodeError),

    #[error("Chain config error: {0}")]
    ChainConfig(String),

    #[error("{0:?}")]
    Services(#[from] crate::services::ServicesError),
}
