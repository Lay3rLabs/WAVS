use utils::storage::db::RedbStorage;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::{host::LogLevel, wavs::aggregator::aggregator::AggregatorAction},
    worlds::aggregator::instance::AggregatorInstanceDepsBuilder,
};
use wavs_types::{Component, ComponentDigest, Packet, ServiceID, WorkflowID};

use crate::helpers::service::make_service;

#[allow(dead_code)]
pub async fn execute_aggregator_component(
    wasm_bytes: &[u8],
    packet: Packet,
) -> Vec<AggregatorAction> {
    let service = make_service(ComponentDigest::hash(wasm_bytes));

    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let data_dir = tempfile::tempdir().unwrap();
    let db_dir = tempfile::tempdir().unwrap();
    let keyvalue_ctx =
        KeyValueCtx::new(RedbStorage::new(db_dir.path()).unwrap(), "test".to_string());

    // dummy aggregator component
    let aggregator_component = Component {
        source: wavs_types::ComponentSource::Digest(ComponentDigest::hash(wasm_bytes)),
        permissions: wavs_types::Permissions::default(),
        fuel_limit: Some(u64::MAX),
        time_limit_seconds: Some(10),
        config: [
            ("chain_name".to_string(), "31337".to_string()),
            (
                "service_handler".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
            ),
        ]
        .into_iter()
        .collect(),
        env_keys: Default::default(),
    };

    let mut instance_deps = AggregatorInstanceDepsBuilder {
        workflow_id: service.workflows.keys().next().cloned().unwrap(),
        service,
        aggregator_component,
        component: WasmtimeComponent::new(&engine, wasm_bytes).unwrap(),
        engine: &engine,
        data_dir: data_dir.path().to_path_buf(),
        chain_configs: &Default::default(),
        log: log_aggregator,
        max_execution_seconds: Some(10),
        max_wasm_fuel: Some(u64::MAX),
        keyvalue_ctx,
    }
    .build()
    .unwrap();

    let aggregator_world =
        wavs_engine::bindings::aggregator::world::AggregatorWorld::instantiate_async(
            &mut instance_deps.store,
            &instance_deps.component,
            &instance_deps.linker,
        )
        .await
        .unwrap();

    let wit_packet = packet.try_into().unwrap();

    let result = aggregator_world
        .call_process_packet(&mut instance_deps.store, &wit_packet)
        .await
        .unwrap();

    match result {
        Ok(actions) => actions,
        Err(e) => panic!("Aggregator component failed: {e}"),
    }
}

#[allow(dead_code)]
fn log_aggregator(
    service_id: &ServiceID,
    workflow_id: &WorkflowID,
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
