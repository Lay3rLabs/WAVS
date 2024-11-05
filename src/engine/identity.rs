use crate::apis::engine::{Engine, EngineError, WasiTask};
use crate::Digest;
use async_trait::async_trait;

/// Simply returns the request as the result.
/// MVP for just testing inputs and outputs and wiring
#[derive(Default, Clone)]
pub struct IdentityEngine;

impl IdentityEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Engine for IdentityEngine {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        Ok(Digest::new(bytecode))
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        Ok(vec![])
    }

    fn get_wasi_task(&self, digest: Digest) -> Result<WasiTask, EngineError> {
        Ok(WasiTask::Mock(digest))
    }

    async fn execute_queue(
        &self,
        _wasi_task: WasiTask,
        request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        Ok(request)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn returns_identity() {
        let engine = IdentityEngine::new();

        // stores and returns unique digest
        let d1 = engine.store_wasm(b"foo").unwrap();
        let d2 = engine.store_wasm(b"bar").unwrap();
        assert_ne!(d1, d2);

        // list doesn't fail
        engine.list_digests().unwrap();

        // execute returns self
        let request = b"this is only a test".to_vec();
        let result = engine
            .execute_queue(WasiTask::Mock(d1), request.clone(), 1234567890)
            .await
            .unwrap();
        assert_eq!(request, result);
    }
}
