use thiserror::Error;
use utils::storage::CAStorageError;
use wavs_types::{Digest, ServiceID, WorkflowID};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error{"Compile: {0}"}]
    Compile(anyhow::Error),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ServiceID, WorkflowID),

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),

    #[error{"{0}"}]
    Engine(#[from] wavs_engine::EngineError),

    #[error{"Unable to send result after executing Service {0} / Workflow {1}"}]
    WasiResultSend(ServiceID, WorkflowID),

    #[error("No registry configured")]
    NoRegistry,

    #[error("{0}")]
    RegistryError(#[from] anyhow::Error),

    #[error("could not encode EventId {0:?}")]
    EncodeEventId(bincode::error::EncodeError),

    #[error("could not encode EventOrder {0:?}")]
    EncodeEventOrder(bincode::error::EncodeError),
}
