use std::collections::BTreeMap;

use alloy_sol_types::SolValue;
use serde::{de::DeserializeOwned, Serialize};
use utils::{storage::db::RedbStorage, test_utils::test_contracts::ISimpleSubmit::DataWithId};
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::operator::world::host::LogLevel,
    utils::error::EngineError,
    worlds::instance::{HostComponentLogger, InstanceDepsBuilder},
};
use wavs_types::{ComponentDigest, EventId, ServiceId, WorkflowId};

use crate::helpers::service::{make_service, make_trigger_action};

#[allow(dead_code)]
pub async fn execute_component<D: DeserializeOwned>(
    wasm_bytes: &[u8],
    config: BTreeMap<String, String>,
    keyvalue_ctx: Option<KeyValueCtx>,
    input: impl Serialize,
) -> D {
    try_execute_component(wasm_bytes, config, keyvalue_ctx, input)
        .await
        .unwrap()
}

#[allow(dead_code)]
pub async fn execute_component_raw(
    engine: WTEngine,
    wasm_bytes: &[u8],
    config: BTreeMap<String, String>,
    keyvalue_ctx: Option<KeyValueCtx>,
    input: Vec<u8>,
) -> Vec<u8> {
    try_execute_component_raw(engine, wasm_bytes, config, keyvalue_ctx, input)
        .await
        .unwrap()
}

#[allow(dead_code)]
pub async fn try_execute_component<D: DeserializeOwned>(
    wasm_bytes: &[u8],
    config: BTreeMap<String, String>,
    keyvalue_ctx: Option<KeyValueCtx>,
    input: impl Serialize,
) -> std::result::Result<D, String> {
    let mut wt_config = WTConfig::new();

    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let res = try_execute_component_raw(
        engine,
        wasm_bytes,
        config,
        keyvalue_ctx,
        serde_json::to_vec(&input).unwrap(),
    )
    .await?;

    let data_with_id: DataWithId = DataWithId::abi_decode(&res).unwrap();
    Ok(serde_json::from_slice::<D>(&data_with_id.data).unwrap())
}

#[allow(dead_code)]
pub async fn try_execute_component_raw(
    engine: WTEngine,
    wasm_bytes: &[u8],
    config: BTreeMap<String, String>,
    keyvalue_ctx: Option<KeyValueCtx>,
    input: Vec<u8>,
) -> std::result::Result<Vec<u8>, String> {
    let service = make_service(ComponentDigest::hash(wasm_bytes), config);
    let trigger_action = make_trigger_action(&service, None, input);

    let event_id: EventId = (&service, &trigger_action)
        .try_into()
        .map_err(|e: anyhow::Error| e.to_string())?;

    let data_dir = tempfile::tempdir().unwrap();
    let keyvalue_ctx = keyvalue_ctx
        .unwrap_or_else(|| KeyValueCtx::new(RedbStorage::new().unwrap(), "test".to_string()));

    let mut instance_deps = InstanceDepsBuilder {
        workflow_id: service.workflows.keys().next().cloned().unwrap(),
        service,
        event_id,
        component: WasmtimeComponent::new(&engine, wasm_bytes).unwrap(),
        engine: &engine,
        data_dir: data_dir.path().to_path_buf(),
        chain_configs: &Default::default(),
        log: HostComponentLogger::OperatorHostComponentLogger(log_wasi),
        keyvalue_ctx,
    }
    .build()
    .unwrap();

    let resp =
        wavs_engine::worlds::operator::execute::execute(&mut instance_deps, trigger_action).await;

    match resp {
        Ok(Some(response)) => Ok(response.payload),
        Ok(None) => Err("No response from component".to_string()),
        Err(e) => {
            match e {
                // return the inner error directly so callers can handle it
                EngineError::ExecResult(err) => Err(err),
                _ => Err(e.to_string()),
            }
        }
    }
}

#[allow(dead_code)]
fn log_wasi(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    level: LogLevel,
    message: String,
) {
    let message = format!("[{service_id}:{workflow_id}:{digest}] {message}");

    match level {
        LogLevel::Error => tracing::error!("{}", message),
        LogLevel::Warn => tracing::warn!("{}", message),
        LogLevel::Info => tracing::info!("{}", message),
        LogLevel::Debug => tracing::debug!("{}", message),
        LogLevel::Trace => tracing::trace!("{}", message),
    }
}
