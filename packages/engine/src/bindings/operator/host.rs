use wavs_types::{AllowedServiceCalls, ChainKey, EventIdSalt};

use crate::worlds::operator::component::OperatorHostComponent;

use super::world::host::{LogLevel, ServiceAndWorkflowId, WorkflowAndWorkflowId};

impl super::world::host::Host for OperatorHostComponent {
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

    fn get_event_id(&mut self, salt: Option<Vec<u8>>) -> Vec<u8> {
        let salt = match salt.as_ref() {
            Some(salt) => EventIdSalt::WasmResponse(salt),
            None => EventIdSalt::Trigger(&self.trigger_data),
        };

        wavs_types::EventId::new(&self.service.id(), &self.workflow_id, salt)
            .unwrap() // very unlikely to happen, would be a bincode error
            .as_bytes()
            .to_vec()
    }

    fn config_var(&mut self, key: String) -> Option<String> {
        self.service
            .workflows
            .get(&self.workflow_id)
            .and_then(|workflow| workflow.component.config.get(&key))
            .cloned()
    }

    fn log(&mut self, level: LogLevel, message: String) {
        let workflow = self
            .service
            .workflows
            .get(&self.workflow_id)
            .unwrap_or_else(|| {
                panic!(
                    "Workflow with ID {} not found in service {}",
                    self.workflow_id,
                    self.service.id()
                )
            });

        let digest = workflow
            .component
            .source
            .digest();

        (self.inner_log)(
            &self.service.id(),
            &self.workflow_id,
            digest,
            level,
            message,
        );
    }

    async fn call_service(
        &mut self,
        callee_id: String,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        const RPC_MAX_DEPTH: usize = 5;

        let caller_service_id = self.service.id().to_string();

        // RPC-02: Caller permission check (AllowedServiceCalls)
        let allowed = match self
            .service
            .workflows
            .get(&self.workflow_id)
            .map(|w| &w.component.permissions.allowed_service_calls)
        {
            Some(AllowedServiceCalls::All) => true,
            Some(AllowedServiceCalls::Only(ids)) => ids.contains(&callee_id),
            Some(AllowedServiceCalls::None) | None => false,
        };
        if !allowed {
            return Err(format!(
                "call-service denied: caller '{}' does not have permission to call '{}'",
                caller_service_id, callee_id
            ));
        }

        // RPC-04: Cycle detection
        if self.call_stack.contains(&callee_id) {
            return Err(format!(
                "call-service cycle detected: '{}' is already in the call chain {:?}",
                callee_id, self.call_stack
            ));
        }

        // RPC-04: Depth limit
        if self.call_stack.len() >= RPC_MAX_DEPTH {
            return Err(format!(
                "call-service depth limit ({}) exceeded: call chain {:?}",
                RPC_MAX_DEPTH, self.call_stack
            ));
        }

        // Get the RPC caller (injected by wavs crate; None means RPC not configured)
        let rpc_caller = self
            .rpc_caller
            .clone()
            .ok_or_else(|| "call-service not available: no RPC caller configured".to_string())?;

        // Thread the call stack — add current service as caller
        let mut new_call_stack = self.call_stack.clone();
        new_call_stack.push(caller_service_id);

        // Delegate to the engine (Plan 02 provides the concrete RpcCaller impl)
        rpc_caller.call(callee_id, payload, new_call_stack).await
    }
}
