use std::sync::RwLock;
use std::{collections::BTreeMap, sync::Arc};

use opentelemetry::global::meter;
use utils::service::DEFAULT_IPFS_GATEWAY;
use utils::storage::{db::WavsDb, fs::FileStorage};
use utils::telemetry::Metrics;
use wavs::subsystems::engine::EngineCommand;
use wavs::{
    services::Services,
    subsystems::{
        engine::{wasm_engine::WasmEngine, EngineManager},
        submission::chain_message::ChainMessage,
    },
};
use wavs_benchmark_common::{app_context::APP_CONTEXT, engine_setup::EngineSetup};
use wavs_types::{Service, TriggerAction};

/// Configuration for the system benchmark (MultiEngineRunner)
#[derive(Clone, Copy)]
pub struct SystemConfig {
    /// Number of concurrent actions to process
    pub n_actions: u64,
}

impl SystemConfig {
    pub fn description(&self) -> String {
        format!("system actions: {}", self.n_actions)
    }
}

/// SystemHandle provides the setup and infrastructure needed for MultiEngineRunner benchmarks
/// This struct combines an EngineHandle with a MultiEngineRunner to test system-level throughput
pub struct SystemSetup {
    pub _engine_setup: Arc<EngineSetup>,
    pub _engine_manager: EngineManager<FileStorage>,
    pub config: SystemConfig,
    pub dispatcher_to_engine_tx: crossbeam::channel::Sender<EngineCommand>,
    pub engine_to_dispatcher_rx: crossbeam::channel::Receiver<ChainMessage>,
    #[allow(clippy::type_complexity)]
    pub trigger_actions: Arc<std::sync::Mutex<Option<Vec<(TriggerAction, Service)>>>>,
}

impl SystemSetup {
    pub fn new(system_config: SystemConfig) -> Arc<Self> {
        let engine_setup = EngineSetup::new(BTreeMap::new());

        // Create file storage for the WasmEngine
        let file_storage = FileStorage::new(engine_setup.data_dir.path().join("ca")).unwrap();

        // Create metrics for the engine
        let metrics = Metrics::new(meter("wavs-benchmark"));

        // Create a WasmEngine similar to how it's done in CoreDispatcher
        let app_storage = engine_setup.data_dir.path().join("app");
        let db_storage = WavsDb::new().unwrap();
        let wasm_engine = WasmEngine::new(
            file_storage,
            app_storage,
            50, // LRU cache size for components
            Arc::new(RwLock::new(engine_setup.chain_configs.clone())),
            None,                // No fuel limit for benchmarks
            None,                // No time limit for benchmarks
            metrics.wavs.engine, // Engine metrics
            db_storage.clone(),
            DEFAULT_IPFS_GATEWAY.to_owned(),
        );

        let digest = wasm_engine
            .store_component_bytes(&engine_setup.component_bytes)
            .unwrap();

        // just a sanity check to ensure the digest matches
        if digest != *engine_setup.workflow().component.source.digest() {
            panic!("Component digest mismatch");
        }

        let (dispatcher_to_engine_tx, dispatcher_to_engine_rx) =
            crossbeam::channel::unbounded::<EngineCommand>();
        let (engine_to_dispatcher_tx, engine_to_dispatcher_rx) =
            crossbeam::channel::unbounded::<ChainMessage>();
        let engine_manager = EngineManager::new(
            wasm_engine,
            Services::new(db_storage),
            dispatcher_to_engine_rx,
            engine_to_dispatcher_tx,
        );

        let trigger_actions = (1..=system_config.n_actions)
            .enumerate()
            .map(|(i, _)| {
                let data = format!("Action number {i}").into_bytes();
                let action = engine_setup.create_trigger_action(data);
                (action, engine_setup.service.clone())
            })
            .collect::<Vec<_>>();

        // Create channels for the Engine Manager pipeline - mirror production pipeline sizes

        // Start the Engine Manager
        std::thread::spawn({
            let engine_manager = engine_manager.clone();
            move || {
                engine_manager.start(APP_CONTEXT.clone());
            }
        });

        Arc::new(SystemSetup {
            _engine_setup: engine_setup,
            _engine_manager: engine_manager,
            config: system_config,
            dispatcher_to_engine_tx,
            engine_to_dispatcher_rx,
            trigger_actions: Arc::new(std::sync::Mutex::new(Some(trigger_actions))),
        })
    }
}
