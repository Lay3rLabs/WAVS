use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use crate::apis::trigger::TriggerData;
use crate::Digest;
use tracing::instrument;

use super::{Engine, EngineError};

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
        self.digests.write().unwrap().insert(digest.clone());

        self.functions
            .write()
            .unwrap()
            .insert(digest.clone(), Box::new(function));
    }
}

impl Engine for MockEngine {
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        let digest = Digest::new(bytecode);
        self.digests.write().unwrap().insert(digest.clone());
        Ok(digest)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        Ok(self.digests.read().unwrap().iter().cloned().collect())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn execute(
        &self,
        component: &crate::apis::dispatcher::Component,
        trigger: &crate::apis::trigger::TriggerAction,
    ) -> Result<Vec<u8>, EngineError> {
        // FIXME: error if it wasn't stored before as well?
        let store = self.functions.read().unwrap();
        let fx = store
            .get(&component.wasm)
            .ok_or(EngineError::UnknownDigest(component.wasm.clone()))?;

        let request = match &trigger.data {
            TriggerData::RawWithId { data, .. } => Ok(data.clone()),
            _ => Err(EngineError::Other(anyhow::anyhow!(
                "Unsupported mock trigger data"
            ))),
        }?;

        let result = fx.execute(request)?;
        Ok(result)
    }
}

pub trait Function: Send + Sync + 'static {
    fn execute(&self, request: Vec<u8>) -> Result<Vec<u8>, EngineError>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn stores_lists() {
        let engine = MockEngine::new();

        // stores and returns unique digest
        let d1 = engine.store_wasm(b"foo").unwrap();
        let d2 = engine.store_wasm(b"bar").unwrap();
        assert_ne!(d1, d2);

        // list contains both digests (in sorted order)
        let list = engine.list_digests().unwrap();
        let mut expected = vec![d1, d2];
        expected.sort();
        assert_eq!(list, expected);
    }

    pub struct FixedResult(Vec<u8>);

    impl Function for FixedResult {
        fn execute(&self, _request: Vec<u8>, _timestamp: u64) -> Result<Vec<u8>, EngineError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn executes_functions() {
        let engine = MockEngine::new();

        // stores and returns unique digest
        let d1 = Digest::new(b"foo");
        let d2 = Digest::new(b"bar");
        let d3 = Digest::new(b"missing");

        // register unique handlers for d1 and d2
        let r1 = b"The first result".to_vec();
        let r2 = b"HDHFOIUHWEOHGFOEHO".to_vec();
        engine.register(&d1, FixedResult(r1.clone()));
        engine.register(&d2, FixedResult(r2.clone()));

        // d1 call gets r1
        let c1 = crate::apis::dispatcher::Component::new(&d1);
        let res = engine
            .execute_queue(
                &c1,
                &ServiceID::new("321").unwrap(),
                TaskId::new(123),
                b"123".into(),
                1234,
            )
            .unwrap();
        assert_eq!(res, r1);

        // d2 call gets r2
        let c2 = crate::apis::dispatcher::Component::new(&d2);
        let res = engine
            .execute_queue(
                &c2,
                &ServiceID::new("321").unwrap(),
                TaskId::new(123),
                b"123".into(),
                1234,
            )
            .unwrap();
        assert_eq!(res, r2);

        // d3 call returns missing error
        let c3 = crate::apis::dispatcher::Component::new(&d3);
        let err = engine
            .execute_queue(
                &c3,
                &ServiceID::new("321").unwrap(),
                TaskId::new(123),
                b"123".into(),
                1234,
            )
            .unwrap_err();
        assert!(matches!(err, EngineError::UnknownDigest(_)));
    }
}
