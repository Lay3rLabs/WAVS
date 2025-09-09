use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing::instrument;

use utils::config::ChainConfigs;
use utils::storage::db::RedbStorage;
use utils::storage::CAStorage;

pub use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::{
    AggregatorAction, SubmitAction,
};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::wavs::types::{chain::AnyTxHash, core::LogLevel},
    common::base_engine::{BaseEngine, BaseEngineConfig},
    worlds::aggregator::instance::{
        AggregatorInstanceDeps as InstanceDeps,
        AggregatorInstanceDepsBuilder as InstanceDepsBuilder,
    },
};
use wavs_types::{Component, ComponentDigest, Packet};

use crate::error::{AggregatorError, AggregatorResult};

pub struct AggregatorEngine<S: CAStorage> {
    engine: BaseEngine<S>,
}

impl<S: CAStorage + Send + Sync + 'static> AggregatorEngine<S> {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        chain_configs: ChainConfigs,
        lru_size: usize,
        max_wasm_fuel: Option<u64>,
        max_execution_seconds: Option<u64>,
        db: RedbStorage,
        storage: Arc<S>,
    ) -> AggregatorResult<Self> {
        let config = BaseEngineConfig {
            app_data_dir: app_data_dir.into(),
            chain_configs,
            lru_size,
            max_wasm_fuel,
            max_execution_seconds,
        };

        let engine = BaseEngine::new(config, db, storage)?;

        Ok(Self { engine })
    }

    pub fn start(&self) -> AggregatorResult<()> {
        self.engine.start_epoch_thread();
        Ok(())
    }

    #[instrument(level = "debug", skip(self, packet, wasm_component), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id))]
    fn create_instance_deps(
        &self,
        component: &Component,
        packet: &Packet,
        wasm_component: wasmtime::component::Component,
    ) -> AggregatorResult<InstanceDeps> {
        let chain_configs = self.engine.get_chain_configs()?;

        InstanceDepsBuilder {
            component: wasm_component,
            aggregator_component: component.clone(),
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &self.engine.wasm_engine,
            data_dir: &self.engine.app_data_dir,
            chain_configs: &chain_configs,
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                LogLevel::Error => tracing::error!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit.or(self.engine.max_wasm_fuel),
            max_execution_seconds: component
                .time_limit_seconds
                .or(self.engine.max_execution_seconds),
            keyvalue_ctx: KeyValueCtx::new(self.engine.db.clone(), packet.service.id().to_string()),
        }
        .build()
        .map_err(Into::into)
    }

    pub async fn execute_packet(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> AggregatorResult<Vec<AggregatorAction>> {
        tracing::info!("Processing packet with custom aggregator component");

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        wavs_engine::worlds::aggregator::execute::execute_packet(&mut instance_deps, packet)
            .await
            .map_err(Into::into)
    }

    async fn load_component(
        &self,
        component: &Component,
    ) -> AggregatorResult<wasmtime::component::Component> {
        self.engine
            .load_component_from_source(&component.source)
            .await
            .map_err(|e| AggregatorError::ComponentLoad(format!("Failed to load component: {}", e)))
    }

    #[instrument(level = "debug", skip(self, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id))]
    pub async fn execute_timer_callback(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> AggregatorResult<Vec<AggregatorAction>> {
        tracing::info!("Handling timer callback with custom aggregator component");

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        wavs_engine::worlds::aggregator::execute::execute_timer_callback(&mut instance_deps, packet)
            .await
            .map_err(Into::into)
    }

    #[instrument(level = "debug", skip(self, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id))]
    pub async fn execute_submit_callback(
        &self,
        component: &Component,
        packet: &Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> AggregatorResult<()> {
        tracing::info!("Handling submit callback with custom aggregator component");

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        wavs_engine::worlds::aggregator::execute::execute_submit_callback(
            &mut instance_deps,
            packet,
            tx_result,
        )
        .await
        .map_err(Into::into)
    }

    pub async fn upload_component(
        &self,
        component_bytes: Vec<u8>,
    ) -> AggregatorResult<ComponentDigest> {
        self.engine
            .store_component_bytes(&component_bytes)
            .map_err(Into::into)
    }
}
