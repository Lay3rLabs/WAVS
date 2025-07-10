use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::instrument;
use utils::config::ChainConfigs;
use utils::storage::db::RedbStorage;
use wasmtime::{component::Component as WasmComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{InstanceDepsBuilder, KeyValueCtx};
use wavs_types::{Component, Packet, ServiceID, WorkflowID};

// those should come from wit bindings
#[derive(Debug, Clone)]
pub enum AggregatorAction {
    Timer {
        delay: u64,
    },
    Submit {
        chain_name: String,
        contract_address: String,
    },
    Nothing,
}

pub struct AggregatorEngine {
    wasm_engine: WTEngine,
    chain_configs: Arc<RwLock<ChainConfigs>>,
    app_data_dir: PathBuf,
    max_wasm_fuel: Option<u64>,
    max_execution_seconds: Option<u64>,
    db: RedbStorage,
}

impl AggregatorEngine {
    pub fn new(
        app_data_dir: impl Into<PathBuf>,
        chain_configs: ChainConfigs,
        max_wasm_fuel: Option<u64>,
        max_execution_seconds: Option<u64>,
        db: RedbStorage,
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
        // Implement the actual engine execution
        // - load the component bytes
        // - create instance dependencies with aggregator-world bindings
        // - instantiate the component with aggregator-world
        // - call process-packet on the component
        // - return the aggregator actions

        tracing::info!("Processing packet with custom aggregator component");

        Ok(vec![])
    }
}
