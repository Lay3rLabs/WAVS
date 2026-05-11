use std::collections::HashMap;

use tempfile::TempDir;
use utils::storage::db::WavsDb;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::operator::world::host::LogLevel,
    rpc::{RpcCaller, RpcFuture},
    utils::error::EngineError,
    worlds::instance::{HostComponentLogger, InstanceData, InstanceDepsBuilder},
};
use wavs_types::{ComponentDigest, ServiceId, WasmResponse, WorkflowId};

use crate::helpers::service::{make_service, make_trigger_action};

/// A test-only RpcCaller that resolves callee services from an in-memory map of
/// callee_id → WASM bytes, executes them inline via the engine, and returns the
/// first response payload.
///
/// This allows engine-level RPC integration tests without importing the `wavs` crate
/// (which would create a circular dependency: `wavs-engine` ← `wavs` → `wavs-engine`).
///
/// The `services` map keys are the exact callee ID strings that the component passes
/// to `call_service`. In tests, these are configured via the `callee_service_id` config var.
pub struct MockRpcCaller {
    pub services: HashMap<String, Vec<u8>>,
}

impl RpcCaller for MockRpcCaller {
    fn call(&self, callee_id: String, payload: Vec<u8>, _call_stack: Vec<String>) -> RpcFuture<'_> {
        // Clone what we need to move into the async block
        let wasm_bytes = self.services.get(&callee_id).cloned();

        Box::pin(async move {
            let wasm_bytes = wasm_bytes
                .ok_or_else(|| format!("MockRpcCaller: unknown service '{}'", callee_id))?;

            // Build a minimal Wasmtime engine for callee execution
            let mut wt_config = WTConfig::new();
            wt_config.wasm_component_model(true);
            wt_config.consume_fuel(true);
            let engine = WTEngine::new(&wt_config)
                .map_err(|e| format!("MockRpcCaller: engine init failed: {}", e))?;

            // Build a minimal service description for the callee
            let callee_service = make_service(ComponentDigest::hash(&wasm_bytes), Default::default());
            let trigger_action = make_trigger_action(&callee_service, None, payload);

            let data_dir = TempDir::new()
                .map_err(|e| format!("MockRpcCaller: tempdir failed: {}", e))?;
            let keyvalue_ctx =
                KeyValueCtx::new(WavsDb::new().unwrap(), "mock-rpc-callee".to_string());

            let component = WasmtimeComponent::new(&engine, &wasm_bytes)
                .map_err(|e| format!("MockRpcCaller: component load failed: {}", e))?;

            let mut instance_deps = InstanceDepsBuilder {
                workflow_id: callee_service.workflows.keys().next().cloned().unwrap(),
                service: callee_service,
                data: InstanceData::new_operator(trigger_action.data.clone()),
                component,
                engine: &engine,
                data_dir: data_dir.path().to_path_buf(),
                chain_configs: &Default::default(),
                log: HostComponentLogger::OperatorHostComponentLogger(log_noop),
                keyvalue_ctx,
                rpc_caller: None,
                call_stack: vec![],
            }
            .build()
            .map_err(|e| format!("MockRpcCaller: build failed: {}", e))?;

            let responses = wavs_engine::worlds::operator::execute::execute(
                &mut instance_deps,
                trigger_action,
                WasmResponse::DEFAULT_MAX_PAYLOAD_SIZE,
                WasmResponse::DEFAULT_MAX_SALT_SIZE,
            )
            .await;

            match responses {
                Ok(mut resps) => resps
                    .pop()
                    .map(|r| r.payload)
                    .ok_or_else(|| "MockRpcCaller: callee returned no responses".to_string()),
                Err(EngineError::ExecResult(err)) => Err(err),
                Err(e) => Err(format!("MockRpcCaller: callee execution failed: {}", e)),
            }
        })
    }
}

fn log_noop(
    _service_id: &ServiceId,
    _workflow_id: &WorkflowId,
    _digest: &ComponentDigest,
    _level: LogLevel,
    _message: String,
) {
}
