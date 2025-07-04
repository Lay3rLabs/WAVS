use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{event, instrument, span};
use utils::config::ChainConfigs;
use utils::telemetry::EngineMetrics;
use utils::wkg::WkgClient;
use wasmtime::{component::Component, Config as WTConfig, Engine as WTEngine};
use wavs_engine::InstanceDepsBuilder;
use wavs_types::{
    ComponentSource, Digest, Service, ServiceID, TriggerAction, WasmResponse, WorkflowID
};

use utils::storage::{CAStorage, CAStorageError};

use super::error::EngineError;

pub struct WasmEngine<S: CAStorage> {
    chain_configs: Arc<RwLock<ChainConfigs>>,
    wasm_storage: S,
    wasm_engine: WTEngine,
    memory_cache: RwLock<LruCache<Digest, Component>>,
    app_data_dir: PathBuf,
    max_wasm_fuel: Option<u64>,
    max_execution_seconds: Option<u64>,
    metrics: EngineMetrics,
}

impl<S: CAStorage> WasmEngine<S> {
    /// Create a new Wasm Engine manager.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        wasm_storage: S,
        app_data_dir: impl AsRef<Path>,
        lru_size: usize,
        chain_configs: ChainConfigs,
        max_wasm_fuel: Option<u64>,
        max_execution_seconds: Option<u64>,
        metrics: EngineMetrics,
    ) -> Self {
        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let wasm_engine = WTEngine::new(&config).unwrap();

        let lru_size = NonZeroUsize::new(lru_size).unwrap();

        let app_data_dir = app_data_dir.as_ref().to_path_buf();

        if !app_data_dir.is_dir() {
            std::fs::create_dir(&app_data_dir).unwrap();
        }

        Self {
            wasm_storage,
            wasm_engine,
            memory_cache: RwLock::new(LruCache::new(lru_size)),
            app_data_dir,
            chain_configs: Arc::new(RwLock::new(chain_configs)),
            max_execution_seconds,
            max_wasm_fuel,
            metrics,
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn start(&self) -> Result<(), EngineError> {
        let engine = self.wasm_engine.clone();

        // just run forever, ticking forward till the end of time (or however long this node is up)
        std::thread::spawn(move || loop {
            engine.increment_epoch();
            std::thread::sleep(Duration::from_secs(1));
        });

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn store_component_bytes(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        // compile component (validate it is proper wasm)
        let cm = Component::new(&self.wasm_engine, bytecode).map_err(EngineError::Compile)?;

        // store original wasm
        let digest = self.wasm_storage.set_data(bytecode)?;

        // // TODO: write precompiled wasm (huge optimization on restart)
        // tokio::fs::write(self.path_for_precompiled_wasm(digest), cm.serialize()?).await?;

        self.memory_cache.write().unwrap().put(digest.clone(), cm);

        Ok(digest)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub async fn store_component_from_source(
        &self,
        source: &ComponentSource,
    ) -> Result<Digest, EngineError> {
        match source {
            // Leaving unimplemented for now, should do validations to confirm bytes
            // really are a wasm component before registring component
            ComponentSource::Download { .. } => todo!(),
            ComponentSource::Registry { registry } => {
                if !(self.wasm_storage.data_exists(&registry.digest)?) {
                    // Fetches package from registry and validates it has the expected digest
                    let client =
                        WkgClient::new(registry.domain.clone().unwrap_or("wa.dev".to_string()))?;
                    let bytes = client.fetch(registry).await?;
                    self.store_component_bytes(&bytes)
                } else {
                    Ok(registry.digest.clone())
                }
            }
            ComponentSource::Digest(digest) => {
                if self.wasm_storage.data_exists(digest)? {
                    Ok(digest.clone())
                } else {
                    self.metrics.increment_total_errors("unknown digest");
                    Err(EngineError::UnknownDigest(digest.clone()))
                }
            }
        }
    }

    // TODO: paginate this
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        let digests: Result<Vec<_>, CAStorageError> = self.wasm_storage.digests()?.collect();
        Ok(digests?)
    }

    /// This will execute a contract that implements the wavs:worker wit interface
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn execute(
        &self,
        service: Service,
        trigger_action: TriggerAction,
    ) -> Result<Option<WasmResponse>, EngineError> {
        fn log(
            service_id: &ServiceID,
            workflow_id: &WorkflowID,
            digest: &Digest,
            level: wavs_engine::bindings::world::host::LogLevel,
            message: String,
        ) {
            let span = span!(
                tracing::Level::INFO,
                "component_log",
                service_id = %service_id,
                workflow_id = %workflow_id,
                digest = %digest
            );

            match level {
                wavs_engine::bindings::world::host::LogLevel::Error => {
                    event!(parent: &span, tracing::Level::ERROR, "{}", message)
                }
                wavs_engine::bindings::world::host::LogLevel::Warn => {
                    event!(parent: &span, tracing::Level::WARN, "{}", message)
                }
                wavs_engine::bindings::world::host::LogLevel::Info => {
                    event!(parent: &span, tracing::Level::INFO, "{}", message)
                }
                wavs_engine::bindings::world::host::LogLevel::Debug => {
                    event!(parent: &span, tracing::Level::DEBUG, "{}", message)
                }
                wavs_engine::bindings::world::host::LogLevel::Trace => {
                    event!(parent: &span, tracing::Level::TRACE, "{}", message)
                }
            }
        }

        let workflow = service
            .workflows
            .get(&trigger_action.config.workflow_id)
            .ok_or_else(|| EngineError::UnknownWorkflow(service.id.clone(), trigger_action.config.workflow_id.clone()))?;

        let digest = workflow.component.source.digest().clone();

        let mut instance_deps = InstanceDepsBuilder {
            service,
            workflow_id: trigger_action.config.workflow_id.clone(),
            component: match self.memory_cache.write().unwrap().get(&digest) {
                Some(cm) => cm.clone(),
                None => {
                    let bytes = self.wasm_storage.get_data(&digest)?;
                    Component::new(&self.wasm_engine, &bytes).map_err(EngineError::Compile)?
                }
            },
            engine: &self.wasm_engine,
            data_dir: self
                .app_data_dir
                .join(trigger_action.config.service_id.as_ref()),
            chain_configs: &self.chain_configs.read().unwrap(),
            log,
            max_execution_seconds: self.max_execution_seconds,
            max_wasm_fuel: self.max_wasm_fuel,
        }
        .build()?;

        self.block_on_run(async move {
            wavs_engine::execute(&mut instance_deps, trigger_action)
                .await
                .map_err(|e| e.into())
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine", service_id = %service_id))]
    pub fn remove_storage(&self, service_id: &ServiceID) {
        let dir_path = self.app_data_dir.join(service_id.as_ref());

        if dir_path.exists() {
            match std::fs::remove_dir_all(&dir_path) {
                Ok(_) => tracing::info!("Successfully removed storage at {:?}", dir_path),
                Err(e) => {
                    self.metrics
                        .increment_total_errors("failed to remove storage");
                    tracing::error!("Failed to remove storage at {:?}: {}", dir_path, e)
                }
            }
        } else {
            tracing::warn!("Storage directory {:?} does not exist", dir_path);
        }
    }

    fn block_on_run<F, T>(&self, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        // Is this necessary? It's a very nuanced and hairy question... see https://github.com/Lay3rLabs/WAVS/issues/224 for details
        // In the meantime, it's reasonable and maybe even optimal even *IF* it's not 100% strictly necessary.
        // TODO: revisit when we have the capability for properly testing throughput under load in different scenarios
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(fut)
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::BTreeMap;

    use utils::{storage::memory::MemoryStorage, test_utils::address::rand_address_evm};
    use wavs_types::{
        ChainName, ServiceID, Submit, Trigger, TriggerConfig, TriggerData, WorkflowID, Workflow,
    };

    use utils::test_utils::{
        address::rand_event_evm,
        mock_chain_configs::mock_chain_configs,
        mock_engine::{COMPONENT_ECHO_DATA, COMPONENT_PERMISSIONS},
    };

    use super::*;

    fn metrics() -> EngineMetrics {
        EngineMetrics::new(&opentelemetry::global::meter("engine-test-metrics"))
    }

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
        );

        // store two blobs
        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();
        let digest2 = engine.store_component_bytes(COMPONENT_PERMISSIONS).unwrap();
        assert_ne!(digest, digest2);

        // list them
        let digests = engine.list_digests().unwrap();
        let mut expected = vec![digest, digest2];
        expected.sort();
        assert_eq!(digests, expected);
    }

    #[test]
    fn reject_invalid_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
        );

        // store valid wasm
        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();
        // fail on invalid wasm
        engine.store_component_bytes(b"foobarbaz").unwrap_err();

        // only list the valid one
        let digests = engine.list_digests().unwrap();
        assert_eq!(digests, vec![digest]);
    }

    #[test]
    fn execute_echo() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
        );

        // store echo digest
        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();

        // also store permissions digest, to test that we execute the right one
        let _ = engine.store_component_bytes(COMPONENT_PERMISSIONS).unwrap();

        let workflow = Workflow {
            trigger: Trigger::evm_contract_event(
                rand_address_evm(),
                ChainName::new("evm").unwrap(),
                rand_event_evm(),
            ),
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        let service_id = ServiceID::new("foobar").unwrap();

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        // execute it and get bytes back
        let result = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"{"x":12}"#),
                },
            )
            .unwrap();

        assert_eq!(&result.unwrap().payload, br#"{"x":12}"#);
    }

    #[test]
    fn validate_execute_config_environment() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
        );

        std::env::set_var("WAVS_ENV_TEST", "testing");
        std::env::set_var("WAVS_ENV_TEST_NOT_ALLOWED", "secret");

        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        workflow.component.env_keys = ["WAVS_ENV_TEST".to_string()].into_iter().collect();
        workflow.component.config = [("foo".to_string(), "bar".to_string())].into();

        let service_id = ServiceID::new("foobar").unwrap();

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        // verify service config kv is accessible
        let result = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"configvar:foo"#),
                },
            )
            .unwrap();

        assert_eq!(&result.unwrap().payload, br#"bar"#);

        // verify whitelisted host env var is accessible
        let result = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:WAVS_ENV_TEST"#),
                },
            )
            .unwrap();

        assert_eq!(&result.unwrap().payload, br#"testing"#);

        // verify the non-enabled env var is not accessible
        let result = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:WAVS_ENV_TEST_NOT_ALLOWED"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            result,
            EngineError::Engine(wavs_engine::EngineError::ExecResult(_))
        ));
    }

    #[test]
    fn execute_without_enough_fuel() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let low_fuel_limit = 1;
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
        );

        // store square digest
        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        workflow.component.fuel_limit = Some(low_fuel_limit);

        let service_id = ServiceID::new("foobar").unwrap();

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        // execute it and get the error
        let err = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(), 
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"{"x":12}"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::EngineError::OutOfFuel(_, _))
        ));
    }

    #[test]
    fn test_remove_storage() {
        // Setup
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let app_data_path = app_data.path().to_path_buf();
        let engine = WasmEngine::new(
            storage,
            &app_data_path,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
        );

        // Create a service ID
        let service_id = ServiceID::new("test-service").unwrap();

        // Create a directory and a test file for the service
        let service_dir = app_data_path.join(service_id.as_ref());
        std::fs::create_dir_all(&service_dir).unwrap();

        let test_file = service_dir.join("test-data.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // Verify directory and file exist
        assert!(service_dir.exists());
        assert!(test_file.exists());

        // Call remove_storage
        engine.remove_storage(&service_id);

        // Verify the directory was removed
        assert!(!service_dir.exists());

        // Test non-existent directory case
        let nonexistent_id = ServiceID::new("nonexistent").unwrap();
        let nonexistent_dir = app_data_path.join(nonexistent_id.as_ref());

        // Verify directory doesn't exist
        assert!(!nonexistent_dir.exists());

        // Call remove_storage on non-existent directory
        engine.remove_storage(&nonexistent_id);

        // Directory should still not exist
        assert!(!nonexistent_dir.exists());
    }

    #[test]
    fn execute_with_low_time_limit() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
        );

        engine.start().unwrap();

        let digest = engine.store_component_bytes(COMPONENT_ECHO_DATA).unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        // first, check that it works with enough time and async sleep
        workflow.component.time_limit_seconds = Some(10);
        workflow
            .component
            .config
            .insert("sleep-ms".to_string(), "1000".to_string());
        workflow
            .component
            .config
            .insert("sleep-kind".to_string(), "async".to_string());

        let service_id = ServiceID::new("foobar").unwrap();

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(), 
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap();

        // now same thing but sync sleep
        workflow.component.time_limit_seconds = Some(10);
        workflow
            .component
            .config
            .insert("sleep-ms".to_string(), "1000".to_string());
        workflow
            .component
            .config
            .insert("sleep-kind".to_string(), "sync".to_string());

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service.id.clone(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap();

        // next, check that it "fails" expectedly with async sleep
        workflow.component.time_limit_seconds = Some(1);
        workflow
            .component
            .config
            .insert("sleep-ms".to_string(), "10000".to_string());
        workflow
            .component
            .config
            .insert("sleep-kind".to_string(), "async".to_string());

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };

        let err = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service_id.clone(), 
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::EngineError::OutOfTime(_, _))
        ));

        // and same thing with sync sleep
        workflow.component.time_limit_seconds = Some(1);
        workflow
            .component
            .config
            .insert("sleep-ms".to_string(), "10000".to_string());
        workflow
            .component
            .config
            .insert("sleep-kind".to_string(), "sync".to_string());

        let service = wavs_types::Service {
            id: service_id.clone(), 
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowID::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "evm".parse().unwrap(), 
                address: Default::default()
            }
        };
        let err = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service_id.clone(), 
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::EngineError::OutOfTime(_, _))
        ));
    }
}
