use std::{collections::BTreeMap, sync::Arc};

use tempfile::{tempdir, TempDir};
use utils::{config::ChainConfigs, filesystem::workspace_path, storage::db::RedbStorage};
use wasmtime::{component::Component, Engine as WTEngine};
use wavs_engine::{
    worlds::operator::component::HostComponentLogger,
    worlds::operator::instance::{InstanceDeps, InstanceDepsBuilder},
};
use wavs_types::{
    AllowedHostPermission, ComponentDigest, Service, TriggerAction, TriggerConfig, TriggerData,
    Workflow, WorkflowId,
};

/// Handle provides the setup and infrastructure needed for engine benchmarks
pub struct EngineSetup {
    pub engine: WTEngine,
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub chain_configs: ChainConfigs,
    pub component: Component,
    pub component_bytes: Vec<u8>,
    pub data_dir: TempDir,
    pub db_dir: TempDir,
    pub keyvalue_ctx: wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx,
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
        let component_source =
            wavs_types::ComponentSource::Digest(ComponentDigest::hash(&component_bytes));
        let component = Component::new(&engine, &component_bytes).unwrap();

        // Create a simple workflow
        let workflow_id = WorkflowId::new("benchmark-workflow".to_string()).unwrap();

        let data_dir = tempdir().unwrap();
        let db_dir = tempdir().unwrap();

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
        };

        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(workflow_id.clone(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "exec".parse().unwrap(),
                address: Default::default(),
            },
        };

        let chain_configs = ChainConfigs::default();

        let keyvalue_ctx = wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx::new(
            RedbStorage::new(db_dir.path()).unwrap(),
            "engine".to_string(),
        );

        Arc::new(EngineSetup {
            engine,
            service,
            component,
            component_bytes,
            workflow_id,
            chain_configs,
            data_dir,
            db_dir,
            keyvalue_ctx,
        })
    }

    pub fn workflow(&self) -> &Workflow {
        self.service
            .workflows
            .get(&self.workflow_id)
            .expect("Workflow not found")
    }

    /// Create a new InstanceDeps for execution
    pub fn create_instance_deps(&self) -> InstanceDeps {
        let log: HostComponentLogger = |_service_id, _workflow_id, _digest, _level, _message| {
            // No-op logger for benchmarks
        };

        let builder = InstanceDepsBuilder {
            component: self.component.clone(),
            service: self.service.clone(),
            workflow_id: self.workflow_id.clone(),
            engine: &self.engine,
            data_dir: self.data_dir.path().to_path_buf(),
            chain_configs: &self.chain_configs,
            log,
            max_wasm_fuel: None,
            max_execution_seconds: None,
            keyvalue_ctx: self.keyvalue_ctx.clone(),
        };

        builder.build().unwrap()
    }

    /// Create a sample TriggerAction for benchmarking
    pub fn create_trigger_action(&self, data: Vec<u8>) -> TriggerAction {
        TriggerAction {
            config: TriggerConfig {
                service_id: self.service.id(),
                workflow_id: self.workflow_id.clone(),
                trigger: wavs_types::Trigger::Manual,
            },
            data: TriggerData::Raw(data),
        }
    }
}
