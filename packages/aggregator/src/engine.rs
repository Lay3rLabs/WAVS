use crate::error::{AggregatorError, AggregatorResult};
use anyhow::Result;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tracing::instrument;
use utils::config::ChainConfigs;
use utils::storage::db::RedbStorage;
use utils::storage::CAStorage;
use wasmtime::{component::Component as WasmComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::aggregator::world::{
        wavs::types::{chain::AnyTxHash, core::LogLevel},
        AggregatorWorld,
    },
    worlds::aggregator::instance::{
        AggregatorInstanceDeps as InstanceDeps,
        AggregatorInstanceDepsBuilder as InstanceDepsBuilder,
    },
};

use utils::wkg::WkgClient;
pub use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::{
    AggregatorAction, SubmitAction,
};
use wavs_types::{Component, ComponentDigest, ComponentSource, Packet};

const MIN_LRU_SIZE: usize = 10;

pub struct AggregatorEngine<S: CAStorage> {
    wasm_engine: WTEngine,
    chain_configs: Arc<RwLock<ChainConfigs>>,
    memory_cache: Mutex<LruCache<ComponentDigest, WasmComponent>>,
    app_data_dir: PathBuf,
    max_wasm_fuel: Option<u64>,
    max_execution_seconds: Option<u64>,
    db: RedbStorage,
    storage: Arc<S>,
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
        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let wasm_engine = WTEngine::new(&config)?;

        let lru_size =
            NonZeroUsize::new(lru_size).unwrap_or(NonZeroUsize::new(MIN_LRU_SIZE).unwrap());
        let app_data_dir = app_data_dir.into();
        if !app_data_dir.is_dir() {
            std::fs::create_dir_all(&app_data_dir)?;
        }

        Ok(Self {
            wasm_engine,
            chain_configs: Arc::new(RwLock::new(chain_configs)),
            memory_cache: Mutex::new(LruCache::new(lru_size)),
            app_data_dir,
            max_wasm_fuel,
            max_execution_seconds,
            db,
            storage,
        })
    }

    pub fn start(&self) -> AggregatorResult<()> {
        let engine = self.wasm_engine.clone();

        std::thread::spawn(move || loop {
            engine.increment_epoch();
            std::thread::sleep(Duration::from_secs(1));
        });

        Ok(())
    }

    #[instrument(level = "debug", skip(self, packet, wasm_component), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    fn create_instance_deps(
        &self,
        component: &Component,
        packet: &Packet,
        wasm_component: WasmComponent,
    ) -> AggregatorResult<InstanceDeps> {
        let chain_configs = self
            .chain_configs
            .read()
            .map_err(|e| anyhow::anyhow!("Chain configs lock poisoned: {}", e))?
            .clone();

        InstanceDepsBuilder {
            component: wasm_component,
            aggregator_component: component.clone(),
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &self.wasm_engine,
            data_dir: &self.app_data_dir,
            chain_configs: &chain_configs,
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                LogLevel::Error => tracing::error!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit.or(self.max_wasm_fuel),
            max_execution_seconds: component.time_limit_seconds.or(self.max_execution_seconds),
            keyvalue_ctx: KeyValueCtx::new(self.db.clone(), packet.service.id().to_string()),
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

        wavs_engine::worlds::aggregator::execute::execute(&mut instance_deps, packet)
            .await
            .map_err(Into::into)
    }

    async fn load_component(&self, component: &Component) -> AggregatorResult<WasmComponent> {
        let digest = component.source.digest().clone();

        let cached_component = {
            // put the lock in a scope so we're careful to drop it here
            let mut lock = self.memory_cache.lock().unwrap();
            lock.get(&digest).cloned()
        };

        if let Some(cached_component) = cached_component {
            return Ok(cached_component);
        }

        // Try to get from storage first, if not found, fetch and store
        let component_bytes = match self.storage.get_data(&digest.clone().into()) {
            Ok(bytes) => bytes,
            Err(e) => {
                // Component not in storage, fetch from source and store it
                match &component.source {
                    ComponentSource::Registry { registry } => {
                        let client = WkgClient::new(
                            registry.domain.clone().unwrap_or("wa.dev".to_string()),
                        )?;
                        let bytes = client.fetch(registry).await?;
                        self.storage.set_data(&bytes)?;
                        bytes
                    }
                    _ => return Err(e.into()),
                }
            }
        };

        let wasm_component = WasmComponent::new(&self.wasm_engine, &component_bytes)?;

        self.memory_cache
            .lock()
            .unwrap()
            .put(digest, wasm_component.clone());

        Ok(wasm_component)
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    pub async fn execute_timer_callback(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> AggregatorResult<Vec<AggregatorAction>> {
        tracing::info!("Handling timer callback with custom aggregator component");

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        let wit_packet = packet.clone().try_into()?;

        let aggregator_world = AggregatorWorld::instantiate_async(
            &mut instance_deps.store,
            &instance_deps.component,
            &instance_deps.linker,
        )
        .await?;

        let result = aggregator_world
            .call_handle_timer_callback(&mut instance_deps.store, &wit_packet)
            .await?;

        match result {
            Ok(actions) => Ok(actions),
            Err(error) => Err(AggregatorError::ComponentExecution(format!(
                "Timer callback execution failed: {}",
                error
            ))),
        }
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    pub async fn execute_submit_callback(
        &self,
        component: &Component,
        packet: &Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> AggregatorResult<()> {
        tracing::info!("Handling submit callback with custom aggregator component");

        let wasm_component = self.load_component(component).await?;
        let mut instance_deps = self.create_instance_deps(component, packet, wasm_component)?;

        let wit_packet = packet.clone().try_into()?;

        let wit_tx_result = tx_result.as_ref().map_err(|e| e.as_str());

        let aggregator_world = AggregatorWorld::instantiate_async(
            &mut instance_deps.store,
            &instance_deps.component,
            &instance_deps.linker,
        )
        .await?;

        let result = aggregator_world
            .call_handle_submit_callback(&mut instance_deps.store, &wit_packet, wit_tx_result)
            .await?;

        match result {
            Ok(_) => Ok(()),
            Err(error) => Err(AggregatorError::ComponentExecution(format!(
                "Submit callback execution failed: {}",
                error
            ))),
        }
    }

    pub async fn upload_component(
        &self,
        component_bytes: Vec<u8>,
    ) -> AggregatorResult<ComponentDigest> {
        // compile component (validate it is proper wasm)
        let cm = WasmComponent::new(&self.wasm_engine, &component_bytes)?;

        // store original wasm
        let digest = ComponentDigest::from(self.storage.set_data(&component_bytes)?.inner());
        self.memory_cache.lock().unwrap().put(digest.clone(), cm);

        Ok(digest)
    }
}
