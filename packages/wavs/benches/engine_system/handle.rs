use std::sync::Arc;

use opentelemetry::global::meter;
use utils::telemetry::Metrics;
use wavs::{engine::{runner::MultiEngineRunner, WasmEngine}, test_utils::address::rand_address_evm};
use wavs_benchmark_common::engine_execute_handle::{EngineHandle, EngineHandleConfig};
use wavs_types::{
    TriggerAction, Service
};
use utils::storage::fs::FileStorage;

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
        format!("system actions: {} (threads: {})", self.n_actions, self.thread_count)
    }
}

/// SystemHandle provides the setup and infrastructure needed for MultiEngineRunner benchmarks
/// This struct combines an EngineHandle with a MultiEngineRunner to test system-level throughput
pub struct SystemHandle {
    pub engine_handle: Arc<EngineHandle>,
    pub multi_runner: MultiEngineRunner<Arc<WasmEngine<FileStorage>>>,
    pub config: SystemConfig,
    pub service: Service,
}

impl SystemHandle {
    pub fn new(system_config: SystemConfig) -> Arc<Self> {
        // Create the base engine handle with a reduced execution count since we'll be doing concurrent work
        let engine_config = EngineHandleConfig {
            n_executions: 1, // Each action gets one execution in the system test
        };
        let engine_handle = EngineHandle::new(engine_config);

        // Create file storage for the WasmEngine
        let file_storage = FileStorage::new(engine_handle.data_dir.path().join("ca")).unwrap();
        
        // Create metrics for the engine
        let metrics = Metrics::new(&meter("wavs-benchmark"));
        
        // Create a WasmEngine similar to how it's done in CoreDispatcher
        let app_storage = engine_handle.data_dir.path().join("app");
        let wasm_engine = Arc::new(WasmEngine::new(
            file_storage,
            app_storage,
            50, // LRU cache size for components
            engine_handle.chain_configs.clone(),
            None, // No registry domain for benchmarks
            None, // No fuel limit for benchmarks  
            None, // No time limit for benchmarks
            metrics.wavs.engine, // Engine metrics
        ));

        // Create the MultiEngineRunner
        let multi_runner = MultiEngineRunner::new(wasm_engine, system_config.thread_count);

        // Create a Service that matches our workflow
        let service = Service {
            id: engine_handle.service_id.clone(),
            name: "Benchmark System Service".to_string(),
            workflows: [(
                engine_handle.workflow_id.clone(),
                engine_handle.workflow.clone(),
            )]
            .into(),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: wavs_types::ChainName::new("benchmark-chain".to_string()).unwrap(),
                address: rand_address_evm(),
            },
        };

        Arc::new(SystemHandle {
            engine_handle,
            multi_runner,
            config: system_config,
            service,
        })
    }

    /// Create multiple TriggerActions for concurrent benchmarking
    pub fn create_trigger_actions(&self) -> Vec<(TriggerAction, Service)> {
        (0..self.config.n_actions)
            .map(|i| {
                let data = format!("System benchmark action {}", i).into_bytes();
                let action = self.engine_handle.create_trigger_action(data);
                (action, self.service.clone())
            })
            .collect()
    }
}