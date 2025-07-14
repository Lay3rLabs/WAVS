use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::instrument;
use utils::config::ChainConfigs;
use utils::storage::db::RedbStorage;
use utils::storage::CAStorage;
use wasmtime::{component::Component as WasmComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{InstanceDepsBuilder, KeyValueCtx};
use wavs_types::{Component, Packet};

// Use the WIT-generated aggregator types
pub use wavs_engine::bindings::world::aggregator::wavs::worker::aggregator::AggregatorAction;
use wavs_engine::bindings::world::aggregator::wavs::worker::aggregator::{
    Packet as WitPacket, TxResult, Envelope as WitEnvelope, EnvelopeSignature as WitEnvelopeSignature, Secp256k1Signature
};
use wavs_engine::bindings::world::aggregator::AggregatorWorld;
use wavs_engine::bindings::world::wavs::worker::helpers::LogLevel;

use wavs_engine::bindings::world::wavs as wavs_world;
use wavs_engine::bindings::world::aggregator::wavs as agg_world;

fn convert_service_to_aggregator(wavs_service: wavs_world::types::service::Service) -> agg_world::types::service::Service {
    unsafe { std::mem::transmute(wavs_service) }
}

fn packet_to_wit_packet(packet: &Packet) -> Result<WitPacket> {
    let wavs_service: wavs_world::types::service::Service = packet.service.clone().try_into()?;
    let wit_service = convert_service_to_aggregator(wavs_service);

    // Convert envelope
    let wit_envelope = WitEnvelope {
        event_id: packet.envelope.eventId.to_vec(),
        ordering: packet.envelope.ordering.to_vec(),
        payload: packet.envelope.payload.to_vec(),
    };

    // Convert signature
    let wit_signature = match &packet.signature {
        wavs_types::EnvelopeSignature::Secp256k1(sig) => {
            WitEnvelopeSignature::Secp256k1(Secp256k1Signature {
                signature_data: sig.as_bytes().to_vec(),
            })
        }
    };

    Ok(WitPacket {
        service: wit_service,
        workflow_id: packet.workflow_id.to_string(),
        envelope: wit_envelope,
        signature: wit_signature,
    })
}

pub struct AggregatorEngine<S: CAStorage> {
    wasm_engine: WTEngine,
    chain_configs: Arc<RwLock<ChainConfigs>>,
    app_data_dir: PathBuf,
    max_wasm_fuel: Option<u64>,
    max_execution_seconds: Option<u64>,
    db: RedbStorage,
    storage: S,
}

impl<S: CAStorage> AggregatorEngine<S> {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        chain_configs: ChainConfigs,
        max_wasm_fuel: Option<u64>,
        max_execution_seconds: Option<u64>,
        db: RedbStorage,
        storage: S,
    ) -> Result<Self> {
        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let wasm_engine = WTEngine::new(&config)?;

        let app_data_dir = app_data_dir.into();
        if !app_data_dir.is_dir() {
            std::fs::create_dir_all(&app_data_dir)?;
        }

        Ok(Self {
            wasm_engine,
            chain_configs: Arc::new(RwLock::new(chain_configs)),
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

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id, workflow_id = %packet.workflow_id))]
    pub async fn process_packet(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> Result<Vec<AggregatorAction>> {
        tracing::info!("Processing packet with custom aggregator component");

        let wasm_component = self.load_component(component)?;

        let mut instance_deps = InstanceDepsBuilder {
            component: wasm_component,
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &self.wasm_engine,
            data_dir: &self.app_data_dir,
            chain_configs: &self.chain_configs.read().unwrap(),
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                LogLevel::Error => tracing::error!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit.or(self.max_wasm_fuel),
            max_execution_seconds: component.time_limit_seconds.or(self.max_execution_seconds),
            keyvalue_ctx: KeyValueCtx::new(self.db.clone(), packet.service.id.to_string()),
        }
        .build()?;

        let wit_packet = packet_to_wit_packet(packet)?;

        let aggregator_world = AggregatorWorld::instantiate_async(
            &mut instance_deps.store,
            &instance_deps.component,
            &instance_deps.linker,
        )
        .await?;

        let result = aggregator_world
            .call_process_packet(&mut instance_deps.store, &wit_packet)
            .await?;

        match result {
            Ok(actions) => Ok(actions),
            Err(error) => {
                tracing::error!("Component execution failed: {}", error);
                anyhow::bail!("Component execution failed: {}", error);
            }
        }
    }

    fn load_component(&self, component: &Component) -> Result<WasmComponent> {
        let digest = component.source.digest().clone();
        let component_bytes = self.storage.get_data(&digest)?;
        let wasm_component = WasmComponent::new(&self.wasm_engine, &component_bytes)?;
        Ok(wasm_component)
    }

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id, workflow_id = %packet.workflow_id))]
    pub async fn handle_timer_callback(
        &self,
        component: &Component,
        packet: &Packet,
    ) -> Result<Vec<AggregatorAction>> {
        tracing::info!("Handling timer callback with custom aggregator component");

        let wasm_component = self.load_component(component)?;

        let mut instance_deps = InstanceDepsBuilder {
            component: wasm_component,
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &self.wasm_engine,
            data_dir: &self.app_data_dir,
            chain_configs: &self.chain_configs.read().unwrap(),
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                LogLevel::Error => tracing::error!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit.or(self.max_wasm_fuel),
            max_execution_seconds: component.time_limit_seconds.or(self.max_execution_seconds),
            keyvalue_ctx: KeyValueCtx::new(self.db.clone(), packet.service.id.to_string()),
        }
        .build()?;

        let wit_packet = packet_to_wit_packet(packet)?;

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

    #[instrument(level = "debug", skip(self, packet), fields(service_id = %packet.service.id, workflow_id = %packet.workflow_id))]
    pub async fn handle_submit_callback(
        &self,
        component: &Component,
        packet: &Packet,
        tx_result: Result<String, String>,
    ) -> Result<bool> {
        tracing::info!("Handling submit callback with custom aggregator component");

        let wasm_component = self.load_component(component)?;

        let mut instance_deps = InstanceDepsBuilder {
            component: wasm_component,
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &self.wasm_engine,
            data_dir: &self.app_data_dir,
            chain_configs: &self.chain_configs.read().unwrap(),
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                LogLevel::Error => tracing::error!("{}", message),
                LogLevel::Warn => tracing::warn!("{}", message),
                LogLevel::Info => tracing::info!("{}", message),
                LogLevel::Debug => tracing::debug!("{}", message),
                LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit.or(self.max_wasm_fuel),
            max_execution_seconds: component.time_limit_seconds.or(self.max_execution_seconds),
            keyvalue_ctx: KeyValueCtx::new(self.db.clone(), packet.service.id.to_string()),
        }
        .build()?;

        let wit_packet = packet_to_wit_packet(packet)?;

        let wit_tx_result = match tx_result {
            Ok(tx_hash) => TxResult::Success(tx_hash),
            Err(error) => TxResult::Error(error),
        };

        let aggregator_world = AggregatorWorld::instantiate_async(
            &mut instance_deps.store,
            &instance_deps.component,
            &instance_deps.linker,
        )
        .await?;

        let result = aggregator_world
            .call_handle_submit_callback(&mut instance_deps.store, &wit_packet, &wit_tx_result)
            .await?;

        match result {
            Ok(success) => Ok(success),
            Err(error) => {
                tracing::error!("Submit callback execution failed: {}", error);
                anyhow::bail!("Submit callback execution failed: {}", error);
            }
        }
    }
}
