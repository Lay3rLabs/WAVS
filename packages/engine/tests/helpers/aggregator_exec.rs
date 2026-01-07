use utils::storage::db::WavsDb;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::{host::LogLevel, wavs::aggregator::output::AggregatorAction},
    worlds::instance::{HostComponentLogger, InstanceData, InstanceDepsBuilder},
};
use wavs_types::{
    AggregatorInput, ChainConfigs, ComponentDigest, EvmChainConfig, Service, ServiceId, WorkflowId,
};

#[allow(dead_code)]
pub async fn execute_aggregator_component(
    wasm_bytes: &[u8],
    input: AggregatorInput,
    service: Service,
) -> Vec<AggregatorAction> {
    let mut wt_config = WTConfig::new();
    wt_config.wasm_component_model(true);
    wt_config.async_support(true);
    wt_config.consume_fuel(true);

    let engine = WTEngine::new(&wt_config).unwrap();

    let data_dir = tempfile::tempdir().unwrap();
    let keyvalue_ctx = KeyValueCtx::new(WavsDb::new().unwrap(), "test".to_string());

    let mut chain_configs = ChainConfigs::default();
    chain_configs
        .add_chain(
            "evm:31337".to_string().parse().unwrap(),
            EvmChainConfig {
                chain_id: "31337".to_string().parse().unwrap(),
                http_endpoint: Some("http://localhost:8545".to_string()),
                faucet_endpoint: None,
                ws_endpoints: vec![],
                ws_priority_endpoint_index: None,
            }
            .into(),
        )
        .unwrap();

    let mut instance_deps = InstanceDepsBuilder {
        workflow_id: input.trigger_action.config.workflow_id.clone(),
        service,
        data: InstanceData::new_aggregator(input.event_id().unwrap()),

        component: WasmtimeComponent::new(&engine, wasm_bytes).unwrap(),
        engine: &engine,
        data_dir: data_dir.path().to_path_buf(),
        chain_configs: &chain_configs,
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

    let wit_input = input.try_into().unwrap();

    let result = aggregator_world
        .call_process_input(instance_deps.store.as_aggregator_mut(), &wit_input)
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
