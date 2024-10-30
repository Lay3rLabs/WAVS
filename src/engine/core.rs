pub use crate::apis::engine::Engine;
use crate::apis::engine::EngineError;

use crate::storage::CAStorage;
use crate::Digest;

pub struct WasmEngine<S: CAStorage> {
    wasm_storage: S,
    // TODO: implement actual wasmtime engine here
}

impl<S: CAStorage> WasmEngine<S> {
    /// Create a new trigger manager.
    /// This returns the manager and a receiver for the trigger actions.
    /// Internally, all triggers may run in an async runtime and send results to the receiver.
    /// Externally, the Dispatcher can read the incoming tasks either sync or async
    pub fn new(wasm_storage: S) -> Self {
        Self { wasm_storage }
    }
}

// TODO: should we make some trait for quicker tasks where you just register closures for the digests?
impl<S: CAStorage> Engine for WasmEngine<S> {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        // TODO: validate bytecode is proper wasm with some wit interface
        let digest = self.wasm_storage.set_data(bytecode)?;
        Ok(digest)
    }

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        // TODO: requires a range query on the castorage (.keys())
        todo!();
    }

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    fn execute_queue(
        &self,
        _digest: Digest,
        _request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        tracing::warn!("EXECUTING TASK QUEUE!");
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::memory::MemoryStorage;

    use super::*;

    const SQUARE: &[u8] = include_bytes!("../../components/square.wasm");
    const BTC_AVG: &[u8] = include_bytes!("../../components/btc_avg.wasm");

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let engine = WasmEngine::new(storage);

        // store two blobs
        let digest = engine.store_wasm(SQUARE).unwrap();
        let digest2 = engine.store_wasm(BTC_AVG).unwrap();
        assert_ne!(digest, digest2);

        // list them
        let digests = engine.list_digests().unwrap();
        let mut expected = vec![digest, digest2];
        expected.sort();
        assert_eq!(digests, expected);
    }

    #[test]
    fn execute_square() {
        let storage = MemoryStorage::new();
        let engine = WasmEngine::new(storage);

        // store square digest
        let digest = engine.store_wasm(SQUARE).unwrap();

        // execute it and get square
        let result = engine
            .execute_queue(digest, br#"{"x":12}"#.into(), 12345)
            .unwrap();
        assert_eq!(&result, br#"{"y":144}"#);
    }
}
