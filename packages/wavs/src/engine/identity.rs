use tracing::instrument;
use wavs_types::{Digest, TriggerAction, WasmResponse, Workflow};

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
    fn store_component_bytes(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        Ok(Digest::new(bytecode))
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    async fn store_component_from_source(
        &self,
        source: &wavs_types::ComponentSource,
    ) -> Result<Digest, EngineError> {
        match source {
            wavs_types::ComponentSource::Download { digest, .. } => {
                Err(EngineError::UnknownDigest(digest.clone()))
            }
            wavs_types::ComponentSource::Registry { registry } => {
                Err(EngineError::UnknownDigest(registry.digest.clone()))
            }
            wavs_types::ComponentSource::Digest(digest) => Ok(digest.clone()),
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        Ok(vec![])
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn execute(
        &self,
        _workflow: Workflow,
        trigger: TriggerAction,
    ) -> Result<Option<WasmResponse>, EngineError> {
        Ok(Some(WasmResponse {
            payload: get_mock_trigger_data(&trigger.data),
            ordering: None,
        }))
    }
}

#[cfg(test)]
mod test {
    use wavs_types::{ComponentSource, Submit, TriggerData};

    use crate::triggers::mock::mock_eth_event_trigger_config;

    use super::*;

    #[test]
    fn returns_identity() {
        let engine = IdentityEngine::new();

        // stores and returns unique digest
        let d1 = engine.store_component_bytes(b"foo").unwrap();
        let d2 = engine.store_component_bytes(b"bar").unwrap();
        assert_ne!(d1, d2);

        // list doesn't fail
        engine.list_digests().unwrap();

        // execute returns self
        let request = b"this is only a test".to_vec();

        let trigger_config = mock_eth_event_trigger_config("foobar", "baz");

        let workflow = Workflow {
            trigger: trigger_config.trigger.clone(),
            component: wavs_types::Component::new(ComponentSource::Digest(d1.clone())),
            submit: Submit::None,
            aggregator: None,
        };
        let result = engine
            .execute(
                workflow,
                TriggerAction {
                    config: trigger_config,
                    data: TriggerData::new_raw(request.clone()),
                },
            )
            .unwrap();
        assert_eq!(request, result.unwrap().payload);
    }
}
