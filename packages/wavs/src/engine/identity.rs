use tracing::instrument;
use wavs_types::{Component, Digest, ServiceConfig, TriggerAction};

use crate::apis::engine::{Engine, EngineError};
use crate::triggers::mock::get_mock_trigger_data;

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
        _service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError> {
        Ok(get_mock_trigger_data(&trigger.data))
    }
}

#[cfg(test)]
mod test {
    use wavs_types::TriggerData;

    use crate::triggers::mock::mock_eth_event_trigger_config;

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
        let component = Component::new(d1);
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: mock_eth_event_trigger_config("foobar", "baz"),
                    data: TriggerData::new_raw(request.clone()),
                },
                &ServiceConfig::default(),
            )
            .unwrap();
        assert_eq!(request, result);
    }
}
