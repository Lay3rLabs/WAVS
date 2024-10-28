use thiserror::Error;

use crate::storage::{CAStorage, CAStorageError};
use crate::Digest;

pub struct WasmEngine<S: CAStorage> {
    wasm_storage: S,
    // TODO: implement actual wasmtime engine here
}

// TODO: should we make some trait for quicker tasks where you just register closures for the digests?
impl<S: CAStorage> WasmEngine<S> {
    /// Create a new trigger manager.
    /// This returns the manager and a receiver for the trigger actions.
    /// Internally, all triggers may run in an async runtime and send results to the receiver.
    /// Externally, the Dispatcher can read the incoming tasks either sync or async
    pub fn new(wasm_storage: S) -> Self {
        Self { wasm_storage }
    }

    pub fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, WasmEngineError> {
        // TODO: validate bytecode is proper wasm with some wit interface
        let digest = self.wasm_storage.set_data(bytecode)?;
        Ok(digest)
    }

    // TODO: paginate this
    pub fn list_digests(&self) -> Result<Vec<Digest>, WasmEngineError> {
        // TODO: requires a range query on the castorage (.keys())
        todo!();
    }

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    pub fn execute_queue(
        &self,
        _digest: Digest,
        _request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, WasmEngineError> {
        todo!();
    }
}

#[derive(Error, Debug)]
pub enum WasmEngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error("Invalid Wasm bytecode")]
    InvalidWasmCode,

    #[error("Wasm doesn't match expected wit interface")]
    WasmInterfaceMismatch,

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),
}
