use std::{collections::BTreeMap, sync::Arc};

use opentelemetry::global::meter;
use utils::storage::{db::RedbStorage, fs::FileStorage};
use utils::telemetry::Metrics;
use wavs::{
    dispatcher::{ENGINE_CHANNEL_SIZE, SUBMISSION_CHANNEL_SIZE},
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
    /// Number of threads for the MultiEngineRunner
    pub thread_count: usize,
}

impl SystemConfig {
    pub fn description(&self) -> String {
        format!(
            "system actions: {} (threads: {})",
            self.n_actions, self.thread_count
        )
    }
}

/// SystemHandle provides the setup and infrastructure needed for MultiEngineRunner benchmarks
/// This struct combines an EngineHandle with a MultiEngineRunner to test system-level throughput
pub struct SystemSetup {
    pub _engine_setup: Arc<EngineSetup>,
    pub _engine_manager: EngineManager<FileStorage>,
    pub config: SystemConfig,
    pub action_sender: tokio::sync::mpsc::Sender<(TriggerAction, Service)>,
    pub result_receiver: std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<ChainMessage>>>,
    pub trigger_actions: std::sync::Mutex<Option<Vec<(TriggerAction, Service)>>>,
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
        let db_storage = RedbStorage::new(engine_setup.data_dir.path().join("db")).unwrap();
        let wasm_engine = WasmEngine::new(
            file_storage,
            app_storage,
            50, // LRU cache size for components
            engine_setup.chain_configs.clone(),
            None,                // No fuel limit for benchmarks
            None,                // No time limit for benchmarks
            metrics.wavs.engine, // Engine metrics
            db_storage.clone(),
        );

        let digest = wasm_engine
            .store_component_bytes(&engine_setup.component_bytes)
            .unwrap();

        // just a sanity check to ensure the digest matches
        if digest != *engine_setup.workflow().component.source.digest() {
            panic!("Component digest mismatch");
        }

        // Create the MultiEngineRunner
        let engine_manager = EngineManager::new(
            wasm_engine,
            system_config.thread_count,
            Services::new(db_storage),
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
        let (action_sender, input_receiver) = tokio::sync::mpsc::channel(ENGINE_CHANNEL_SIZE);
        let (result_sender, result_receiver) = tokio::sync::mpsc::channel(SUBMISSION_CHANNEL_SIZE);

        // Start the Engine Manager
        engine_manager.start(APP_CONTEXT.clone(), input_receiver, result_sender);

        Arc::new(SystemSetup {
            _engine_setup: engine_setup,
            _engine_manager: engine_manager,
            config: system_config,
            action_sender,
            result_receiver: std::sync::Mutex::new(Some(result_receiver)),
            trigger_actions: std::sync::Mutex::new(Some(trigger_actions)),
        })
    }
}
