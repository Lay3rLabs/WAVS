use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use crate::apis::{dispatcher::ServiceConfig, trigger::TriggerAction};
use crate::triggers::mock::get_mock_trigger_data;
use crate::Digest;
use tracing::instrument;
use utils::config::{ChainConfigs, CosmosChainConfig, EthereumChainConfig};

use super::{Engine, EngineError};

/// Maintains a list of the digests that have been stored.
/// You can also register Functions with any of the digests and it will be run
/// when that digest is called
#[derive(Default, Clone)]
pub struct MockEngine {
    digests: Arc<RwLock<BTreeSet<Digest>>>,
    functions: Arc<RwLock<BTreeMap<Digest, Box<dyn Function>>>>,
}

pub fn mock_chain_configs() -> ChainConfigs {
    ChainConfigs {
        eth: vec![(
            "eth".to_string(),
            EthereumChainConfig {
                chain_id: 31337.to_string(),
                ws_endpoint: Some("ws://localhost:8546".to_string()),
                http_endpoint: Some("http://localhost:8545".to_string()),
                aggregator_endpoint: None,
                faucet_endpoint: None,
            },
        )]
        .into_iter()
        .collect(),
        cosmos: vec![(
            "cosmos".to_string(),
            CosmosChainConfig {
                chain_id: "cosmos".to_string(),
                rpc_endpoint: Some("http://localhost:26657".to_string()),
                grpc_endpoint: Some("http://localhost:9090".to_string()),
                bech32_prefix: "cosmos".to_string(),
                gas_denom: "ustake".to_string(),
                gas_price: 0.025,
                faucet_endpoint: None,
            },
        )]
        .into_iter()
        .collect(),
    }
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
        trigger: TriggerAction,
        _service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError> {
        // FIXME: error if it wasn't stored before as well?
        let store = self.functions.read().unwrap();
        let fx = store
            .get(&component.wasm)
            .ok_or(EngineError::UnknownDigest(component.wasm.clone()))?;
        let result = fx.execute(get_mock_trigger_data(&trigger.data))?;
        Ok(result)
    }
}

pub trait Function: Send + Sync + 'static {
    fn execute(&self, request: Vec<u8>) -> Result<Vec<u8>, EngineError>;
}

#[cfg(test)]
mod test {
    use crate::test_utils::address::rand_event_eth;

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
        fn execute(&self, _request: Vec<u8>) -> Result<Vec<u8>, EngineError> {
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
        let c1 = crate::apis::dispatcher::Component::new(d1);
        let res = engine
            .execute(
                &c1,
                TriggerAction {
                    config: crate::apis::trigger::TriggerConfig::eth_contract_event(
                        "321",
                        "default",
                        crate::test_utils::address::rand_address_eth(),
                        "eth",
                        rand_event_eth(),
                    )
                    .unwrap(),
                    data: crate::apis::trigger::TriggerData::new_raw(b"123"),
                },
                &ServiceConfig::default(),
            )
            .unwrap();
        assert_eq!(res, r1);

        // d2 call gets r2
        let c2 = crate::apis::dispatcher::Component::new(d2);
        let res = engine
            .execute(
                &c2,
                TriggerAction {
                    config: crate::apis::trigger::TriggerConfig::eth_contract_event(
                        "321",
                        "default",
                        crate::test_utils::address::rand_address_eth(),
                        "eth",
                        rand_event_eth(),
                    )
                    .unwrap(),
                    data: crate::apis::trigger::TriggerData::new_raw(b"123"),
                },
                &ServiceConfig::default(),
            )
            .unwrap();
        assert_eq!(res, r2);

        // d3 call returns missing error
        let c3 = crate::apis::dispatcher::Component::new(d3);
        let err = engine
            .execute(
                &c3,
                TriggerAction {
                    config: crate::apis::trigger::TriggerConfig::eth_contract_event(
                        "321",
                        "default",
                        crate::test_utils::address::rand_address_eth(),
                        "eth",
                        rand_event_eth(),
                    )
                    .unwrap(),
                    data: crate::apis::trigger::TriggerData::new_raw(b"123"),
                },
                &ServiceConfig::default(),
            )
            .unwrap_err();
        assert!(matches!(err, EngineError::UnknownDigest(_)));
    }
}
