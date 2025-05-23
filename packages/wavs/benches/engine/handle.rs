use std::sync::{Arc, LazyLock};

use tempfile::{tempdir, TempDir};
use utils::{config::ChainConfigs, context::AppContext, filesystem::workspace_path};
use wasmtime::{component::Component, Engine as WTEngine};
use wavs_engine::{HostComponentLogger, InstanceDeps, InstanceDepsBuilder};
use wavs_types::{
    AllowedHostPermission, Digest, ServiceID, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowID
};

/// Shared application context for benchmarks

pub static APP_CONTEXT: LazyLock<AppContext> = LazyLock::new(AppContext::new);

/// Configuration for the engine benchmark
#[derive(Clone, Copy)]
pub struct HandleConfig {
    /// Number of executions to perform
    pub n_executions: u64,
}

impl HandleConfig {
    pub fn description(&self) -> String {
        format!("engine executions: {}", self.n_executions)
    }
}

/// Handle provides the setup and infrastructure needed for engine benchmarks
pub struct Handle {
    pub engine: WTEngine,
    pub workflow: Workflow,
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub chain_configs: ChainConfigs,
    pub config: HandleConfig,
    pub component: Component,
    pub data_dir: TempDir
}

impl Handle {
    pub fn new(handle_config: HandleConfig) -> Arc<Self> {
        // Create wasmtime engine
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = WTEngine::new(&config).unwrap();

        // Load the echo_raw.wasm component
        let component_path = workspace_path().join("examples").join("build").join("components").join("echo_raw.wasm");
        let component_bytes = std::fs::read(&component_path).unwrap();
        let component_source = wavs_types::ComponentSource::Digest(Digest::new(&component_bytes));
        let component = Component::new(&engine, &component_bytes).unwrap();

        // Create a simple workflow
        let service_id = ServiceID::new("benchmark-service".to_string()).unwrap();
        let workflow_id = WorkflowID::new("benchmark-workflow".to_string()).unwrap();

        let data_dir = tempdir().unwrap();
        
        let workflow = Workflow {
            trigger: wavs_types::Trigger::Manual,
            component: wavs_types::Component {
                source: component_source,
                permissions: wavs_types::Permissions {
                    file_system: false,
                    allowed_http_hosts: AllowedHostPermission::None,
                },
                fuel_limit: None, 
                time_limit_seconds: None,
                config: std::collections::BTreeMap::new(),
                env_keys: std::collections::BTreeSet::new(),
            },
            submit: wavs_types::Submit::None,
            aggregators: Vec::new(),
        };

        let chain_configs = ChainConfigs::default();

        Arc::new(Handle {
            engine,
            workflow,
            component,
            service_id,
            workflow_id,
            chain_configs,
            config: handle_config,
            data_dir,
        })
    }

    /// Create a new InstanceDeps for execution
    pub fn create_instance_deps(&self) -> anyhow::Result<InstanceDeps> {
        let log: HostComponentLogger = |_service_id, _workflow_id, _digest, _level, _message| {
            // No-op logger for benchmarks
        };

        let builder = InstanceDepsBuilder {
            component: self.component.clone(),
            workflow: self.workflow.clone(),
            service_id: self.service_id.clone(),
            workflow_id: self.workflow_id.clone(),
            engine: &self.engine,
            data_dir: self.data_dir.path().to_path_buf(), 
            chain_configs: &self.chain_configs,
            log,
            max_wasm_fuel: None,
            max_execution_seconds: None,
        };

        Ok(builder.build()?)
    }

    /// Create a sample TriggerAction for benchmarking
    pub fn create_trigger_action(&self, data: Vec<u8>) -> TriggerAction {
        TriggerAction {
            config: TriggerConfig {
                service_id: self.service_id.clone(),
                workflow_id: self.workflow_id.clone(),
                trigger: wavs_types::Trigger::Manual,
            },
            data: TriggerData::Raw(data)
        }
    }
}