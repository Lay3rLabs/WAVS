use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

// We can provide a mock implementation of the trait here for easier testing.
use crate::apis::engine::{Engine, EngineError};

use crate::Digest;

/// Maintains a list of the digests that have been stored.
/// You can also register Functions with any of the digests and it will be run
/// when that digest is called
#[derive(Default, Clone)]
pub struct MockEngine {
    digests: Arc<RwLock<BTreeSet<Digest>>>,
    functions: Arc<RwLock<BTreeMap<Digest, Box<dyn Function>>>>,
}

impl MockEngine {
    pub fn new() -> Self {
        MockEngine {
            digests: Arc::new(RwLock::new(BTreeSet::new())),
            functions: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    pub fn register(&self, digest: &Digest, function: impl Function) {
        self.functions
            .write()
            .unwrap()
            .insert(digest.clone(), Box::new(function));
    }
}

impl Engine for MockEngine {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        let digest = Digest::new(bytecode);
        self.digests.write().unwrap().insert(digest.clone());
        Ok(digest)
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        Ok(self.digests.read().unwrap().iter().cloned().collect())
    }

    fn execute_queue(
        &self,
        digest: Digest,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        let store = self.functions.read().unwrap();
        let fx = store
            .get(&digest)
            .ok_or(EngineError::UnknownDigest(digest))?;
        let result = fx.execute(request, timestamp)?;
        Ok(result)
    }
}

pub trait Function: Send + Sync + 'static {
    fn execute(&self, request: Vec<u8>, timestamp: u64) -> Result<Vec<u8>, EngineError>;
}
