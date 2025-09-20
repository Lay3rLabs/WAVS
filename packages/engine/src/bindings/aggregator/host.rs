use wavs_types::ChainKey;

use crate::worlds::aggregator::component::AggregatorHostComponent;

use super::world::host::Host;
use super::world::wavs::types::core::LogLevel;
use super::world::wavs::types::service::{ServiceAndWorkflowId, WorkflowAndWorkflowId};

impl Host for AggregatorHostComponent {
    fn get_cosmos_chain_config(
        &mut self,
        chain: String,
    ) -> Option<super::world::wavs::types::chain::CosmosChainConfig> {
        let chain = ChainKey::new(chain).ok()?;

        self.chain_configs
            .get_chain(&chain)?
            .to_cosmos_config()
            .ok()
            .map(Into::into)
    }

    fn get_evm_chain_config(
        &mut self,
        chain: String,
    ) -> Option<super::world::wavs::types::chain::EvmChainConfig> {
        let chain = ChainKey::new(chain).ok()?;

        self.chain_configs
            .get_chain(&chain)?
            .to_evm_config()
            .ok()
            .map(Into::into)
    }

    fn config_var(&mut self, key: String) -> Option<String> {
        self.service
            .workflows
            .get(&self.workflow_id)
            .and_then(|workflow| match &workflow.submit {
                wavs_types::Submit::Aggregator { component, .. } => component.config.get(&key),
                _ => unreachable!(),
            })
            .cloned()
    }

    fn get_service(&mut self) -> ServiceAndWorkflowId {
        ServiceAndWorkflowId {
            service: self.service.clone().try_into().unwrap(),
            workflow_id: self.workflow_id.to_string(),
        }
    }

    fn get_workflow(&mut self) -> WorkflowAndWorkflowId {
        let workflow = self
            .service
            .workflows
            .get(&self.workflow_id)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "Workflow with ID {} not found in service {}",
                    self.workflow_id,
                    self.service.id()
                )
            });
        WorkflowAndWorkflowId {
            workflow: workflow.try_into().unwrap(),
            workflow_id: self.workflow_id.to_string(),
        }
    }

    fn log(&mut self, level: LogLevel, message: String) {
        let digest = self
            .service
            .workflows
            .get(&self.workflow_id)
            .and_then(|workflow| match &workflow.submit {
                wavs_types::Submit::Aggregator { component, .. } => Some(component.source.digest()),
                _ => unreachable!(),
            })
            .unwrap_or_else(|| {
                panic!(
                    "Workflow with ID {} not found in service {}",
                    self.workflow_id,
                    self.service.id()
                )
            });

        (self.inner_log)(
            &self.service.id(),
            &self.workflow_id,
            digest,
            level,
            message,
        );
    }
}
