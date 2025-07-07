use std::{collections::BTreeMap, sync::Arc};

use tempfile::{tempdir, TempDir};
use utils::{config::ChainConfigs, filesystem::workspace_path};
use wasmtime::{component::Component, Engine as WTEngine};
use wasmtime_wasi_keyvalue;
use wavs_engine::{HostComponentLogger, InstanceDeps, InstanceDepsBuilder};
use wavs_types::{
    AllowedHostPermission, Digest, ServiceID, TriggerAction, TriggerConfig, TriggerData, Workflow,
    WorkflowID,
};

/// Handle provides the setup and infrastructure needed for engine benchmarks
pub struct EngineSetup {
    pub engine: WTEngine,
    pub workflow: Workflow,
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub chain_configs: ChainConfigs,
    pub component: Component,
    pub component_bytes: Vec<u8>,
    pub data_dir: TempDir,
}

impl EngineSetup {
    pub fn new(config: BTreeMap<String, String>) -> Arc<Self> {
        // Create wasmtime engine
        let mut wt_config = wasmtime::Config::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);
        wt_config.epoch_interruption(true);
        let engine = WTEngine::new(&wt_config).unwrap();

        // Load the echo_data.wasm component
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("echo_data.wasm");
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
                config,
                env_keys: std::collections::BTreeSet::new(),
            },
            submit: wavs_types::Submit::None,
            aggregators: Vec::new(),
        };

        let chain_configs = ChainConfigs::default();

        Arc::new(EngineSetup {
            engine,
            workflow,
            component,
            component_bytes,
            service_id,
            workflow_id,
            chain_configs,
            data_dir,
        })
    }

    /// Create a new InstanceDeps for execution
    pub fn create_instance_deps(&self) -> InstanceDeps {
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
            keyvalue_ctx: std::sync::Arc::new(
                wasmtime_wasi_keyvalue::WasiKeyValueCtxBuilder::new().build(),
            ),
        };

        builder.build().unwrap()
    }

    /// Create a sample TriggerAction for benchmarking
    pub fn create_trigger_action(&self, data: Vec<u8>) -> TriggerAction {
        TriggerAction {
            config: TriggerConfig {
                service_id: self.service_id.clone(),
                workflow_id: self.workflow_id.clone(),
                trigger: wavs_types::Trigger::Manual,
            },
            data: TriggerData::Raw(data),
        }
    }
}
