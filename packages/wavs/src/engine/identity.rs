use tracing::instrument;

use crate::apis::dispatcher::Component;
use crate::apis::engine::{Engine, EngineError};

use crate::Digest;

/// Simply returns the request as the result.
/// MVP for just testing inputs and outputs and wiring
#[derive(Default, Clone)]
pub struct IdentityEngine;

impl IdentityEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Engine for IdentityEngine {
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        Ok(Digest::new(bytecode))
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        Ok(vec![])
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn execute_queue(
        &self,
        _component: &Component,
        request: Vec<u8>,
        _timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        Ok(request)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn returns_identity() {
        let engine = IdentityEngine::new();

        // stores and returns unique digest
        let d1 = engine.store_wasm(b"foo").unwrap();
        let d2 = engine.store_wasm(b"bar").unwrap();
        assert_ne!(d1, d2);

        // list doesn't fail
        engine.list_digests().unwrap();

        // execute returns self
        let request = b"this is only a test".to_vec();
        let component = Component::new(&d1);
        let result = engine
            .execute_queue(&component, request.clone(), 1234567890)
            .unwrap();
        assert_eq!(request, result);
    }
}
