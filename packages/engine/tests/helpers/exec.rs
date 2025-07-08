use alloy_sol_types::SolValue;
use serde::{de::DeserializeOwned, Serialize};
use utils::test_utils::test_contracts::ISimpleSubmit::DataWithId;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{bindings::world::host::LogLevel, InstanceDepsBuilder};
use wavs_types::{Digest, ServiceID, WorkflowID};

use crate::helpers::service::{make_service, make_trigger_action};

pub async fn execute_component<D: DeserializeOwned>(wasm_bytes: &[u8], input: impl Serialize) -> D {
    let service = make_service(Digest::new(wasm_bytes));
    let trigger_action = make_trigger_action(&service, None, serde_json::to_vec(&input).unwrap());

    let mut wt_config = WTConfig::new();

    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let data_dir = tempfile::tempdir().unwrap();

    let mut instance_deps = InstanceDepsBuilder {
        workflow_id: service.workflows.keys().next().cloned().unwrap(),
        service,
        component: WasmtimeComponent::new(&engine, wasm_bytes).unwrap(),
        engine: &engine,
        data_dir: data_dir.path().to_path_buf(),
        chain_configs: &Default::default(),
        log: log_wasi,
        max_execution_seconds: Some(u64::MAX),
        max_wasm_fuel: Some(u64::MAX),
    }
    .build()
    .unwrap();

    let payload = wavs_engine::execute(&mut instance_deps, trigger_action)
        .await
        .unwrap()
        .unwrap()
        .payload;
    let data_with_id: DataWithId = DataWithId::abi_decode(&payload).unwrap();
    serde_json::from_slice(&data_with_id.data).unwrap()
}

fn log_wasi(
    service_id: &ServiceID,
    workflow_id: &WorkflowID,
    digest: &Digest,
    level: LogLevel,
    message: String,
) {
    let message = format!("[{}:{}:{}] {}", service_id, workflow_id, digest, message);

    match level {
        LogLevel::Error => tracing::error!("{}", message),
        LogLevel::Warn => tracing::warn!("{}", message),
        LogLevel::Info => tracing::info!("{}", message),
        LogLevel::Debug => tracing::debug!("{}", message),
        LogLevel::Trace => tracing::trace!("{}", message),
    }
}
