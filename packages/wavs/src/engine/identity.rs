use tracing::instrument;
use wavs_types::{Digest, ServiceConfig, TriggerAction};

use crate::apis::engine::{Engine, EngineError, ExecutionComponent};
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
        _component: &ExecutionComponent,
        _fuel_limit: Option<u64>,
        trigger: TriggerAction,
        _service_config: &ServiceConfig,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        Ok(Some(get_mock_trigger_data(&trigger.data)))
    }
}

#[cfg(test)]
mod test {
    use wavs_types::{Permissions, TriggerData};

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
        let execution_component = ExecutionComponent {
            wasm: d1,
            permissions: Permissions::default(),
        };
        let result = engine
            .execute(
                &execution_component,
                None,
                TriggerAction {
                    config: mock_eth_event_trigger_config("foobar", "baz"),
                    data: TriggerData::new_raw(request.clone()),
                },
                &ServiceConfig::default(),
            )
            .unwrap();
        assert_eq!(request, result.unwrap());
    }
}
