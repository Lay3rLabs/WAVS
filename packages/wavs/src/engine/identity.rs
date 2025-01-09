use tracing::instrument;

use crate::apis::dispatcher::{Component, ServiceConfig};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::trigger::TriggerAction;
use crate::Digest;
use utils::{ServiceID, WorkflowID};

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
    fn execute(
        &self,
        _component: &Component,
        trigger: TriggerAction,
        _service_config: &ServiceConfig
    ) -> Result<Vec<u8>, EngineError> {
        Ok(trigger.data.into_vec().unwrap())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        apis::{
            dispatcher::ComponentWorld,
            trigger::{TriggerConfig, TriggerData},
        },
        test_utils::address::rand_address_eth,
    };

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
        let component = Component::new(d1, ComponentWorld::Raw);
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig::contract_event(
                        "foobar",
                        "baz",
                        rand_address_eth(),
                        "eth",
                    )
                    .unwrap(),
                    data: TriggerData::new_raw(request.clone()),
                },
                &ServiceConfig::default()
            )
            .unwrap();
        assert_eq!(request, result);
    }
}
