use anyhow::Result;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
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
    worlds::aggregator::instance::AggregatorInstanceDepsBuilder as InstanceDepsBuilder,
};

pub use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::{
    AggregatorAction, SubmitAction,
};
use wavs_types::{AnyDigest, Component, ComponentDigest, Packet};

const MIN_LRU_SIZE: usize = 10;

pub struct AggregatorEngine<S: CAStorage> {
    wasm_engine: WTEngine,
    chain_configs: Arc<RwLock<ChainConfigs>>,
    memory_cache: RwLock<LruCache<ComponentDigest, WasmComponent>>,
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
    ) -> Result<Self> {
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
            memory_cache: RwLock::new(LruCache::new(lru_size)),
            app_data_dir,
            max_wasm_fuel,
            max_execution_seconds,
            db,
            storage,
        })
    }

    pub fn start(&self) -> Result<()> {
        let engine = self.wasm_engine.clone();

        std::thread::spawn(move || loop {
            engine.increment_epoch();
            std::thread::sleep(Duration::from_secs(1));
        });

        Ok(())
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    pub async fn process_packet(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> Result<Vec<AggregatorAction>> {
        tracing::info!("Processing packet with custom aggregator component");

        let wasm_component = self.load_component(component)?;
        let chain_configs = self
            .chain_configs
            .read()
            .map_err(|e| anyhow::anyhow!("Chain configs lock poisoned: {}", e))?
            .clone();

        let mut instance_deps = InstanceDepsBuilder {
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
        .build()?;

        wavs_engine::worlds::aggregator::execute::execute(&mut instance_deps, packet)
            .await
            .map_err(Into::into)
    }

    fn load_component(&self, component: &Component) -> Result<WasmComponent> {
        let digest = component.source.digest().clone();

        if let Some(cached_component) = self
            .memory_cache
            .write()
            .map_err(|e| anyhow::anyhow!("Memory cache lock poisoned: {}", e))?
            .get(&digest)
        {
            return Ok(cached_component.clone());
        }

        let bytes: [u8; 32] = digest.as_ref().try_into().map_err(|_| {
            anyhow::anyhow!(
                "Invalid digest length: expected 32 bytes, got {}",
                digest.as_ref().len()
            )
        })?;
        let any_digest = AnyDigest::from(bytes);
        let component_bytes = self.storage.get_data(&any_digest)?;
        let wasm_component = WasmComponent::new(&self.wasm_engine, &component_bytes)?;

        self.memory_cache
            .write()
            .map_err(|e| anyhow::anyhow!("Memory cache lock poisoned: {}", e))?
            .put(digest, wasm_component.clone());

        Ok(wasm_component)
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    pub async fn handle_timer_callback(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> Result<Vec<AggregatorAction>> {
        tracing::info!("Handling timer callback with custom aggregator component");

        let wasm_component = self.load_component(component)?;
        let chain_configs = self
            .chain_configs
            .read()
            .map_err(|e| anyhow::anyhow!("Chain configs lock poisoned: {}", e))?
            .clone();

        let mut instance_deps = InstanceDepsBuilder {
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
        .build()?;

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
            Err(error) => {
                tracing::error!("Timer callback execution failed: {}", error);
                anyhow::bail!("Timer callback execution failed: {}", error);
            }
        }
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
    pub async fn handle_submit_callback(
        &self,
        component: &Component,
        packet: &Packet,
        tx_result: Result<String, String>,
    ) -> Result<bool> {
        tracing::info!("Handling submit callback with custom aggregator component");

        let wasm_component = self.load_component(component)?;
        let chain_configs = self
            .chain_configs
            .read()
            .map_err(|e| anyhow::anyhow!("Chain configs lock poisoned: {}", e))?
            .clone();

        let mut instance_deps = InstanceDepsBuilder {
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
        .build()?;

        let wit_packet = packet.clone().try_into()?;

        let wit_tx_hash = tx_result
            .as_ref()
            .map(|s| match s.parse::<alloy_primitives::TxHash>() {
                Ok(tx_hash) => AnyTxHash::Evm(tx_hash.to_vec()),
                Err(_) => AnyTxHash::Cosmos(s.clone()),
            });

        let wit_tx_result = wit_tx_hash.as_ref().map_err(|e| e.as_str());

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
            Ok(success) => Ok(success),
            Err(error) => {
                tracing::error!("Submit callback execution failed: {}", error);
                anyhow::bail!("Submit callback execution failed: {}", error);
            }
        }
    }

    pub async fn upload_component(&self, component_bytes: Vec<u8>) -> Result<ComponentDigest> {
        // compile component (validate it is proper wasm)
        let cm = WasmComponent::new(&self.wasm_engine, &component_bytes)?;

        // store original wasm
        let digest = self.storage.set_data(&component_bytes)?;

        let bytes: [u8; 32] = digest.as_ref().try_into().map_err(|_| {
            anyhow::anyhow!(
                "Invalid digest length: expected 32 bytes, got {}",
                digest.as_ref().len()
            )
        })?;
        let component_digest = ComponentDigest::from(bytes);
        self.memory_cache
            .write()
            .map_err(|e| anyhow::anyhow!("Memory cache lock poisoned: {}", e))?
            .put(component_digest.clone(), cm);

        Ok(component_digest)
    }
}
