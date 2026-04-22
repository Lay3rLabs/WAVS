use std::sync::Arc;

use wavs_engine::rpc::{RpcCaller, RpcFuture};
use wavs_types::{AllowedCallers, ServiceId, Trigger, TriggerAction, TriggerConfig, TriggerData};

use crate::services::Services;

use super::wasm_engine::WasmEngine;
use utils::storage::CAStorage;

/// Concrete RpcCaller implementation for the wavs crate.
///
/// Holds both the engine and the services registry. Constructed in EngineManager
/// where both are available, avoiding the need to add Services to WasmEngine.
///
/// Responsibilities:
/// - Parse and resolve the callee service ID from the registry
/// - Enforce callee-side AllowedCallers permission check (RPC-03)
/// - Build a synthetic TriggerAction with TriggerData::Raw(payload)
/// - Call execute_operator_component_with_rpc to thread the call stack
///   into the callee's OperatorHostComponent
pub struct RpcCallerImpl<S: CAStorage> {
    pub engine: Arc<WasmEngine<S>>,
    pub services: Services,
}

impl<S: CAStorage + Send + Sync + 'static> RpcCaller for RpcCallerImpl<S> {
    fn call(
        &self,
        callee_id: String,
        payload: Vec<u8>,
        call_stack: Vec<String>,
    ) -> RpcFuture<'_> {
        Box::pin(async move {
            // Parse callee service ID from its hex string representation
            let callee_service_id: ServiceId = callee_id
                .parse()
                .map_err(|e| format!("call-service: invalid callee service ID '{}': {}", callee_id, e))?;

            // Resolve callee service from registry
            let callee_service = self
                .services
                .get(&callee_service_id)
                .map_err(|e| format!("call-service: callee service '{}' not found: {}", callee_id, e))?;

            // RPC-03: Callee-side AllowedCallers check
            // The caller_id is the last item pushed onto the call stack by the caller's
            // call_service host function (which appends caller_service_id before delegating).
            let caller_id = call_stack
                .last()
                .ok_or_else(|| "call-service: empty call stack (internal error)".to_string())?;

            // Use the first workflow for RPC dispatch (lexicographic order for BTreeMap)
            let callee_workflow = callee_service
                .workflows
                .values()
                .next()
                .ok_or_else(|| {
                    format!("call-service: callee '{}' has no workflows", callee_id)
                })?;

            // Enforce callee-side permission: default None rejects all callers
            let callee_accepts = match &callee_workflow.component.allowed_callers {
                Some(AllowedCallers::All) => true,
                Some(AllowedCallers::Only(ids)) => ids.contains(caller_id),
                Some(AllowedCallers::None) | None => false,
            };

            if !callee_accepts {
                return Err(format!(
                    "call-service denied: callee '{}' does not accept calls from '{}'",
                    callee_id, caller_id
                ));
            }

            // Determine callee workflow ID (first in BTreeMap, lexicographic)
            let callee_workflow_id = callee_service
                .workflows
                .keys()
                .next()
                .expect("already verified callee has at least one workflow")
                .clone();

            // Build a synthetic TriggerAction: TriggerData::Raw carries the RPC payload.
            // Trigger::Manual is used as the placeholder trigger type.
            let trigger_action = TriggerAction {
                config: TriggerConfig {
                    service_id: callee_service_id,
                    workflow_id: callee_workflow_id,
                    trigger: Trigger::Manual,
                },
                data: TriggerData::Raw(payload),
            };

            // Construct a new RpcCallerImpl for nested calls so the callee's
            // OperatorHostComponent also gets an injected rpc_caller.
            let nested_rpc = Arc::new(RpcCallerImpl {
                engine: self.engine.clone(),
                services: self.services.clone(),
            });

            // Execute the callee component with the extended call stack
            let responses = self
                .engine
                .execute_operator_component_with_rpc(
                    callee_service,
                    trigger_action,
                    Some(nested_rpc),
                    call_stack,
                )
                .await
                .map_err(|e| format!("call-service execution failed: {}", e))?;

            // Return the first response payload; the WIT contract returns a single bytes blob
            responses
                .into_iter()
                .next()
                .map(|r| r.payload)
                .ok_or_else(|| "call-service: callee returned no responses".to_string())
        })
    }
}
