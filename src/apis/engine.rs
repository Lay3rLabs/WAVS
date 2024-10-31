use thiserror::Error;

use crate::{storage::CAStorageError, Digest};

pub trait Engine: Send + Sync {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError>;

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    fn execute_queue(
        &self,
        _digest: Digest,
        _request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, EngineError>;
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
        digest: Digest,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        self.as_ref().execute_queue(digest, request, timestamp)
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
