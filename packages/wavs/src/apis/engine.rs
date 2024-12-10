use alloy::primitives::Address;
use thiserror::Error;

use crate::{storage::CAStorageError, Digest};

use super::{dispatcher::Component, ID};

pub trait Engine: Send + Sync {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError>;

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    fn execute_queue(
        &self,
        component: &Component,
        request: EngineRequest,
    ) -> Result<Vec<u8>, EngineError>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum EngineRequest {
    CosmosTaskQueue {
        input: Vec<u8>,
        timestamp: u64,
    },
    EthEvent {
        log_address: Address,
        event_topics: Vec<Vec<u8>>,
        event_data: Vec<u8>,
    },
}

impl EngineRequest {
    pub fn cosmos_task_queue(input: Vec<u8>, timestamp: u64) -> Self {
        Self::CosmosTaskQueue { input, timestamp }
    }
}

impl<E: Engine> Engine for std::sync::Arc<E> {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        self.as_ref().store_wasm(bytecode)
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        self.as_ref().list_digests()
    }

    fn execute_queue(
        &self,
        component: &Component,
        request: EngineRequest,
    ) -> Result<Vec<u8>, EngineError> {
        self.as_ref().execute_queue(component, request)
    }
}

// Note: I tried to pull this into an associated type of the trait,
// But then embedding this in DispatcherError got all kinds of ugly.
// We need to use anyhow if we want to allow this to be a trait associated type
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error{"IO: {0}"}]
    IO(#[from] std::io::Error),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ID, ID),

    #[error("Unknown Component {0}")]
    UnknownComponent(ID),

    #[error("Invalid Wasm bytecode")]
    InvalidWasmCode,

    #[error("Wasm doesn't match expected wit interface")]
    WasmInterfaceMismatch,

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),

    #[error("Component returned an error: {0}")]
    ComponentError(String),

    #[error{"{0}"}]
    Other(#[from] anyhow::Error),
}
