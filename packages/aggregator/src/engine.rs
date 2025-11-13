use std::sync::Arc;
use std::{path::PathBuf, sync::RwLock};

use anyhow::Result;
use tracing::instrument;

use utils::storage::db::WavsDb;
use utils::storage::CAStorage;

pub use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::{
    AggregatorAction, SubmitAction,
};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::wavs::types::{chain::AnyTxHash, core::LogLevel},
    common::base_engine::{BaseEngine, BaseEngineConfig},
    worlds::instance::{HostComponentLogger, InstanceDeps, InstanceDepsBuilder},
};
use wavs_types::{ChainConfigs, Component, ComponentDigest, Packet};

use crate::error::{AggregatorError, AggregatorResult};

pub struct AggregatorEngine<S: CAStorage> {
    engine: BaseEngine<S>,
    metrics: utils::telemetry::AggregatorMetrics,
}

impl<S: CAStorage + Send + Sync + 'static> AggregatorEngine<S> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        chain_configs: Arc<RwLock<ChainConfigs>>,
        lru_size: usize,
        max_wasm_fuel: Option<u64>,
        max_execution_seconds: Option<u64>,
        db: WavsDb,
        storage: Arc<S>,
        metrics: utils::telemetry::AggregatorMetrics,
        ipfs_gateway: String,
    ) -> AggregatorResult<Self> {
        let config = BaseEngineConfig {
            app_data_dir: app_data_dir.into(),
            chain_configs,
            lru_size,
            max_wasm_fuel,
            max_execution_seconds,
            ipfs_gateway,
        };

        let engine = BaseEngine::new(config, db, storage)?;

        Ok(Self { engine, metrics })
    }

    #[instrument(skip(self, packet, wasm_component), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id, event_id = %packet.event_id()))]
    fn create_instance_deps(
        &self,
        component: &Component,
        packet: &Packet,
        wasm_component: wasmtime::component::Component,
    ) -> AggregatorResult<InstanceDeps> {
        let chain_configs = self.engine.get_chain_configs()?;

        InstanceDepsBuilder {
            component: wasm_component,
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            event_id: packet.event_id(),
            engine: &self.engine.wasm_engine,
            data_dir: &self.engine.app_data_dir,
            chain_configs: &chain_configs,
            log: HostComponentLogger::AggregatorHostComponentLogger(
                |_service_id, _workflow_id, _digest, level, message| match level {
                    LogLevel::Error => tracing::error!("{}", message),
                    LogLevel::Warn => tracing::warn!("{}", message),
                    LogLevel::Info => tracing::info!("{}", message),
                    LogLevel::Debug => tracing::debug!("{}", message),
                    LogLevel::Trace => tracing::trace!("{}", message),
                },
            ),
            keyvalue_ctx: KeyValueCtx::new(self.engine.db.clone(), packet.service.id().to_string()),
        }
        .build()
        .map_err(Into::into)
    }

    #[instrument(skip(self, component, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id, event_id = %packet.event_id()))]
    pub async fn execute_packet(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> AggregatorResult<Vec<AggregatorAction>> {
        tracing::info!("Processing packet with custom aggregator component");

        #[cfg(debug_assertions)]
        if std::env::var("WAVS_FORCE_AGGREGATOR_ENGINE_ERROR_XXX").is_ok() {
            self.metrics.engine.executions_failed.add(1, &[]);
            self.metrics.engine.total_errors.add(1, &[]);
            return Err(AggregatorError::ComponentLoad(
                "Forced aggregator engine error for testing alerts".into(),
            ));
        }

        let start_time = std::time::Instant::now();

        #[cfg(debug_assertions)]
        if std::env::var("WAVS_FORCE_SLOW_AGGREGATOR_ENGINE_XXX").is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(6));
        }

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        let fuel_before = instance_deps.store.get_fuel().unwrap_or(0);

        let result =
            wavs_engine::worlds::aggregator::execute::execute_packet(&mut instance_deps, packet)
                .await;

        let fuel_after = instance_deps.store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);
        self.metrics
            .engine
            .fuel_consumption
            .record(fuel_consumed, &[]);

        let duration = start_time.elapsed().as_secs_f64();
        self.metrics.engine.execution_duration.record(duration, &[]);

        match result {
            Ok(actions) => {
                self.metrics.engine.executions_success.add(1, &[]);
                Ok(actions)
            }
            Err(e) => {
                self.metrics.engine.executions_failed.add(1, &[]);
                self.metrics.engine.total_errors.add(1, &[]);
                Err(e.into())
            }
        }
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

    #[instrument(skip(self, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id, event_id = %packet.event_id()))]
    pub async fn execute_timer_callback(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> AggregatorResult<Vec<AggregatorAction>> {
        tracing::info!("Handling timer callback with custom aggregator component");

        let start_time = std::time::Instant::now();
        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        let fuel_before = instance_deps.store.get_fuel().unwrap_or(0);

        let result = wavs_engine::worlds::aggregator::execute::execute_timer_callback(
            &mut instance_deps,
            packet,
        )
        .await;

        let fuel_after = instance_deps.store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);
        self.metrics
            .engine
            .fuel_consumption
            .record(fuel_consumed, &[]);

        let duration = start_time.elapsed().as_secs_f64();
        self.metrics.engine.execution_duration.record(duration, &[]);

        match result {
            Ok(actions) => {
                self.metrics.engine.executions_success.add(1, &[]);
                Ok(actions)
            }
            Err(e) => {
                self.metrics.engine.executions_failed.add(1, &[]);
                self.metrics.engine.total_errors.add(1, &[]);
                Err(e.into())
            }
        }
    }

    #[instrument(skip(self, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id, event_id = %packet.event_id()))]
    pub async fn execute_submit_callback(
        &self,
        component: &Component,
        packet: &Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> AggregatorResult<()> {
        tracing::info!("Handling submit callback with custom aggregator component");

        let start_time = std::time::Instant::now();
        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        let fuel_before = instance_deps.store.get_fuel().unwrap_or(0);

        let result = wavs_engine::worlds::aggregator::execute::execute_submit_callback(
            &mut instance_deps,
            packet,
            tx_result,
        )
        .await;

        let fuel_after = instance_deps.store.get_fuel().unwrap_or(0);
        let fuel_consumed = fuel_before.saturating_sub(fuel_after);
        self.metrics
            .engine
            .fuel_consumption
            .record(fuel_consumed, &[]);

        let duration = start_time.elapsed().as_secs_f64();
        self.metrics.engine.execution_duration.record(duration, &[]);

        match result {
            Ok(()) => {
                self.metrics.engine.executions_success.add(1, &[]);
                Ok(())
            }
            Err(e) => {
                self.metrics.engine.executions_failed.add(1, &[]);
                self.metrics.engine.total_errors.add(1, &[]);
                Err(e.into())
            }
        }
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
