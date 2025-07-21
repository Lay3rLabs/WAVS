use wavs_types::ChainName;

use crate::worlds::worker::component::HostComponent;

use super::world::host::{LogLevel, ServiceAndWorkflowId, WorkflowAndWorkflowId};

impl super::world::host::Host for HostComponent {
    fn get_cosmos_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::host::CosmosChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .cosmos
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }

    fn get_evm_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::host::EvmChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .evm
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
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

    fn config_var(&mut self, key: String) -> Option<String> {
        self.service
            .workflows
            .get(&self.workflow_id)
            .and_then(|workflow| workflow.component.config.get(&key))
            .cloned()
    }

    fn log(&mut self, level: LogLevel, message: String) {
        let digest = self
            .service
            .workflows
            .get(&self.workflow_id)
            .map(|workflow| workflow.component.source.digest())
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
