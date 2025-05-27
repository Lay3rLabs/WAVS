use std::sync::Arc;

use opentelemetry::global::meter;
use utils::storage::fs::FileStorage;
use utils::telemetry::Metrics;
use wavs::{
    apis::submission::{ChainMessage, Submission},
    engine::{
        runner::{EngineRunner, MultiEngineRunner},
        Engine, WasmEngine,
    },
    submission::core::CoreSubmission,
    test_utils::address::rand_address_evm,
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
    pub _multi_runner: MultiEngineRunner<Arc<WasmEngine<FileStorage>>>,
    pub config: SystemConfig,
    pub action_sender: tokio::sync::mpsc::Sender<(TriggerAction, Service)>,
    pub result_receiver: std::sync::Mutex<Option<tokio::sync::mpsc::Receiver<ChainMessage>>>,
    pub trigger_actions: std::sync::Mutex<Option<Vec<(TriggerAction, Service)>>>,
}

impl SystemSetup {
    pub fn new(system_config: SystemConfig) -> Arc<Self> {
        let engine_setup = EngineSetup::new();

        // Create file storage for the WasmEngine
        let file_storage = FileStorage::new(engine_setup.data_dir.path().join("ca")).unwrap();

        // Create metrics for the engine
        let metrics = Metrics::new(&meter("wavs-benchmark"));

        // Create a WasmEngine similar to how it's done in CoreDispatcher
        let app_storage = engine_setup.data_dir.path().join("app");
        let wasm_engine = Arc::new(WasmEngine::new(
            file_storage,
            app_storage,
            50, // LRU cache size for components
            engine_setup.chain_configs.clone(),
            None,                // No fuel limit for benchmarks
            None,                // No time limit for benchmarks
            metrics.wavs.engine, // Engine metrics
        ));

        let digest = wasm_engine
            .store_component_bytes(&engine_setup.component_bytes)
            .unwrap();

        // just a sanity check to ensure the digest matches
        if digest != *engine_setup.workflow.component.source.digest() {
            panic!("Component digest mismatch");
        }

        // Create the MultiEngineRunner
        let multi_runner = MultiEngineRunner::new(wasm_engine, system_config.thread_count);

        // Create a Service that matches our workflow
        let service = Service {
            id: engine_setup.service_id.clone(),
            name: "Benchmark System Service".to_string(),
            workflows: [(
                engine_setup.workflow_id.clone(),
                engine_setup.workflow.clone(),
            )]
            .into(),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: wavs_types::ChainName::new("benchmark-chain".to_string()).unwrap(),
                address: rand_address_evm(),
            },
        };

        let trigger_actions = (1..=system_config.n_actions)
            .enumerate()
            .map(|(i, _)| {
                let data = format!("Action number {}", i).into_bytes();
                let action = engine_setup.create_trigger_action(data);
                (action, service.clone())
            })
            .collect::<Vec<_>>();

        // Create channels for the MultiEngineRunner pipeline - mirror production pipeline sizes
        let (action_sender, input_receiver) =
            tokio::sync::mpsc::channel(WasmEngine::<FileStorage>::CHANNEL_SIZE);
        let (result_sender, result_receiver) =
            tokio::sync::mpsc::channel(CoreSubmission::CHANNEL_SIZE);

        // Start the MultiEngineRunner
        multi_runner.start(APP_CONTEXT.clone(), input_receiver, result_sender);

        Arc::new(SystemSetup {
            _engine_setup: engine_setup,
            _multi_runner: multi_runner,
            config: system_config,
            action_sender,
            result_receiver: std::sync::Mutex::new(Some(result_receiver)),
            trigger_actions: std::sync::Mutex::new(Some(trigger_actions)),
        })
    }
}
