use utils::storage::db::RedbStorage;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::{host::LogLevel, wavs::aggregator::aggregator::AggregatorAction},
    worlds::instance::{HostComponentLogger, InstanceDepsBuilder},
};
use wavs_types::{ComponentDigest, Packet, ServiceId, WorkflowId};

#[allow(dead_code)]
pub async fn execute_aggregator_component(
    wasm_bytes: &[u8],
    packet: Packet,
) -> Vec<AggregatorAction> {
    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let data_dir = tempfile::tempdir().unwrap();
    let keyvalue_ctx = KeyValueCtx::new(RedbStorage::new().unwrap(), "test".to_string());

    let mut instance_deps = InstanceDepsBuilder {
        workflow_id: packet.workflow_id.clone(),
        service: packet.service.clone(),
        event_id: packet.event_id(),
        component: WasmtimeComponent::new(&engine, wasm_bytes).unwrap(),
        engine: &engine,
        data_dir: data_dir.path().to_path_buf(),
        chain_configs: &Default::default(),
        log: HostComponentLogger::AggregatorHostComponentLogger(log_aggregator),
        keyvalue_ctx,
    }
    .build()
    .unwrap();

    let aggregator_world =
        wavs_engine::bindings::aggregator::world::AggregatorWorld::instantiate_async(
            instance_deps.store.as_aggregator_mut(),
            &instance_deps.component,
            instance_deps.linker.as_aggregator_ref(),
        )
        .await
        .unwrap();

    let wit_packet = packet.try_into().unwrap();

    let result = aggregator_world
        .call_process_packet(instance_deps.store.as_aggregator_mut(), &wit_packet)
        .await
        .unwrap();

    match result {
        Ok(actions) => actions,
        Err(e) => panic!("Aggregator component failed: {e}"),
    }
}

#[allow(dead_code)]
fn log_aggregator(
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
