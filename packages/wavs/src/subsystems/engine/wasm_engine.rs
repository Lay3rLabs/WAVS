use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{event, instrument, span};
use utils::config::ChainConfigs;
use utils::storage::db::RedbStorage;
use utils::telemetry::EngineMetrics;
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    common::base_engine::{BaseEngine, BaseEngineConfig},
    worlds::operator::instance::InstanceDepsBuilder,
};
use wavs_types::{
    ComponentDigest, ComponentSource, Service, ServiceId, TriggerAction, WasmResponse, WorkflowId,
};

use utils::storage::CAStorage;

use super::error::EngineError;

pub struct WasmEngine<S: CAStorage> {
    engine: BaseEngine<S>,
    metrics: EngineMetrics,
}

impl<S: CAStorage + Send + Sync + 'static> WasmEngine<S> {
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
        db: RedbStorage,
    ) -> Self {
        let config = BaseEngineConfig {
            app_data_dir: app_data_dir.as_ref().to_path_buf(),
            chain_configs,
            lru_size,
            max_wasm_fuel,
            max_execution_seconds,
        };

        let engine = BaseEngine::new(config, db, Arc::new(wasm_storage)).unwrap();

        Self { engine, metrics }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn start(&self) -> Result<(), EngineError> {
        self.engine.start_epoch_thread();
        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub fn store_component_bytes(&self, bytecode: &[u8]) -> Result<ComponentDigest, EngineError> {
        Ok(self.engine.store_component_bytes(bytecode)?)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    pub async fn store_component_from_source(
        &self,
        source: &ComponentSource,
    ) -> Result<ComponentDigest, EngineError> {
        match source {
            // Leaving unimplemented for now, should do validations to confirm bytes
            // really are a wasm component before registring component
            ComponentSource::Download { .. } => todo!(),
            ComponentSource::Registry { registry } => {
                if !(self
                    .engine
                    .storage
                    .data_exists(&registry.digest.clone().into())?)
                {
                    // Fetches package from registry and validates it has the expected digest
                    let _component = self.engine.load_component_from_source(source).await?;
                    Ok(registry.digest.clone())
                } else {
                    Ok(registry.digest.clone())
                }
            }
            ComponentSource::Digest(digest) => {
                if self.engine.storage.data_exists(&digest.clone().into())? {
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
    pub fn list_digests(&self) -> Result<Vec<ComponentDigest>, EngineError> {
        let digests: Result<Vec<_>, _> = self
            .engine
            .storage
            .digests()?
            .map(|d| d.map(|d| ComponentDigest::from(d.inner())))
            .collect();
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
            service_id: &ServiceId,
            workflow_id: &WorkflowId,
            digest: &ComponentDigest,
            level: wavs_engine::bindings::operator::world::host::LogLevel,
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
                wavs_engine::bindings::operator::world::host::LogLevel::Error => {
                    event!(parent: &span, tracing::Level::ERROR, "{}", message)
                }
                wavs_engine::bindings::operator::world::host::LogLevel::Warn => {
                    event!(parent: &span, tracing::Level::WARN, "{}", message)
                }
                wavs_engine::bindings::operator::world::host::LogLevel::Info => {
                    event!(parent: &span, tracing::Level::INFO, "{}", message)
                }
                wavs_engine::bindings::operator::world::host::LogLevel::Debug => {
                    event!(parent: &span, tracing::Level::DEBUG, "{}", message)
                }
                wavs_engine::bindings::operator::world::host::LogLevel::Trace => {
                    event!(parent: &span, tracing::Level::TRACE, "{}", message)
                }
            }
        }

        let workflow = service
            .workflows
            .get(&trigger_action.config.workflow_id)
            .ok_or_else(|| {
                EngineError::UnknownWorkflow(
                    service.id(),
                    trigger_action.config.workflow_id.clone(),
                )
            })?;

        let digest = workflow.component.source.digest().clone();
        let chain_configs = self.engine.get_chain_configs()?;

        let component = self.block_on_run(async { self.engine.load_component(&digest).await })?;

        let service_id = service.id();
        let workflow_id = trigger_action.config.workflow_id.clone();

        let mut instance_deps = InstanceDepsBuilder {
            keyvalue_ctx: KeyValueCtx::new(self.engine.db.clone(), service.id().to_string()),
            service,
            workflow_id: trigger_action.config.workflow_id.clone(),
            component,
            engine: &self.engine.wasm_engine,
            data_dir: self
                .engine
                .app_data_dir
                .join(trigger_action.config.service_id.to_string()),
            chain_configs: &chain_configs,
            log,
            max_execution_seconds: self.engine.max_execution_seconds,
            max_wasm_fuel: self.engine.max_wasm_fuel,
        }
        .build()?;

        let initial_fuel = instance_deps.store.get_fuel().unwrap_or(0);
        let start_time = Instant::now();

        let (result, final_fuel) = self.block_on_run(async move {
            let result =
                wavs_engine::worlds::operator::execute::execute(&mut instance_deps, trigger_action)
                    .await;
            let final_fuel = instance_deps.store.get_fuel().unwrap_or(0);
            (result, final_fuel)
        });

        let duration = start_time.elapsed().as_secs_f64();
        let fuel_consumed = initial_fuel.saturating_sub(final_fuel);

        self.metrics.record_execution(
            duration,
            fuel_consumed,
            &service_id.to_string(),
            workflow_id.as_ref(),
            result.is_ok(),
        );

        tracing::info!(
            service_id = %service_id,
            workflow_id = %workflow_id,
            duration_seconds = duration,
            fuel_consumed = fuel_consumed,
            success = result.is_ok(),
            "WASM execution completed"
        );

        result.map_err(|e| e.into())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Engine", service_id = %service_id))]
    pub fn remove_storage(&self, service_id: &ServiceId) {
        let dir_path = self.engine.app_data_dir.join(service_id.to_string());

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
        ChainName, ServiceId, Submit, Trigger, TriggerConfig, TriggerData, Workflow, WorkflowId,
    };

    use utils::test_utils::{
        address::rand_event_evm,
        mock_chain_configs::mock_chain_configs,
        mock_engine::{COMPONENT_ECHO_DATA_BYTES, COMPONENT_PERMISSIONS_BYTES},
    };

    use super::*;

    fn metrics() -> EngineMetrics {
        EngineMetrics::new(&opentelemetry::global::meter("engine-test-metrics"))
    }

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let db_dir = tempfile::tempdir().unwrap();

        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        // store two blobs
        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();
        let digest2 = engine
            .store_component_bytes(COMPONENT_PERMISSIONS_BYTES)
            .unwrap();
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
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        // store valid wasm
        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();
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
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        // store echo digest
        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();

        // also store permissions digest, to test that we execute the right one
        let _ = engine
            .store_component_bytes(COMPONENT_PERMISSIONS_BYTES)
            .unwrap();

        let workflow = Workflow {
            trigger: Trigger::evm_contract_event(
                rand_address_evm(),
                ChainName::new("evm").unwrap(),
                rand_event_evm(),
            ),
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
        };

        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();
        // execute it and get bytes back
        let result = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowId::default(),
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
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        std::env::set_var("WAVS_ENV_TEST", "testing");
        std::env::set_var("WAVS_ENV_TEST_NOT_ALLOWED", "secret");

        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
        };

        workflow.component.env_keys = ["WAVS_ENV_TEST".to_string()].into_iter().collect();
        workflow.component.config = [("foo".to_string(), "bar".to_string())].into();

        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        // verify service config kv is accessible
        let result = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service_id.clone(),
                        workflow_id: WorkflowId::default(),
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
                        service_id: service_id.clone(),
                        workflow_id: WorkflowId::default(),
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
                        service_id,
                        workflow_id: WorkflowId::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:WAVS_ENV_TEST_NOT_ALLOWED"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            result,
            EngineError::Engine(wavs_engine::utils::error::EngineError::ExecResult(_))
        ));
    }

    #[test]
    fn execute_without_enough_fuel() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let low_fuel_limit = 1;
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        // store square digest
        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
        };

        workflow.component.fuel_limit = Some(low_fuel_limit);

        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        // execute it and get the error
        let err = engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowId::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"{"x":12}"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::utils::error::EngineError::OutOfFuel(_, _))
        ));
    }

    #[test]
    fn test_remove_storage() {
        // Setup
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let app_data_path = app_data.path().to_path_buf();
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data_path,
            3,
            ChainConfigs::default(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        // Create a service ID
        let service_id = ServiceId::hash(b"test-service");

        // Create a directory and a test file for the service
        let service_dir = app_data_path.join(service_id.to_string());
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
        let nonexistent_id = ServiceId::hash("nonexistent");
        let nonexistent_dir = app_data_path.join(nonexistent_id.to_string());

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
        let db_dir = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(
            storage,
            &app_data,
            3,
            mock_chain_configs(),
            None,
            None,
            metrics(),
            RedbStorage::new(db_dir.path()).unwrap(),
        );

        engine.start().unwrap();

        let digest = engine
            .store_component_bytes(COMPONENT_ECHO_DATA_BYTES)
            .unwrap();
        let mut workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component::new(ComponentSource::Digest(digest.clone())),
            submit: Submit::None,
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

        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id: service_id.clone(),
                        workflow_id: WorkflowId::default(),
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
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        engine
            .execute(
                service.clone(),
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowId::default(),
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
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow.clone())]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        let err = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowId::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::utils::error::EngineError::OutOfTime(_, _))
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
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain_name: "evm".parse().unwrap(),
                address: Default::default(),
            },
        };

        let service_id = service.id();

        let err = engine
            .execute(
                service,
                TriggerAction {
                    config: TriggerConfig {
                        service_id,
                        workflow_id: WorkflowId::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"hello world"#),
                },
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Engine(wavs_engine::utils::error::EngineError::OutOfTime(_, _))
        ));
    }
}
