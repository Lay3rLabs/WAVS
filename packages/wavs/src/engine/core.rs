use anyhow::Context;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::instrument;
use utils::config::ChainConfigs;
use utils::types::{AllowedHostPermission, ServiceConfig};
use wasmtime::{
    component::{Component, Linker},
    Config as WTConfig, Engine as WTEngine,
};
use wasmtime::{Store, Trap};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::apis::trigger::{TriggerAction, TriggerError};
use utils::digest::Digest;
use utils::storage::{CAStorage, CAStorageError};

use super::{Engine, EngineError};

pub struct WasmEngine<S: CAStorage> {
    chain_configs: ChainConfigs,
    wasm_storage: S,
    wasm_engine: WTEngine,
    memory_cache: RwLock<LruCache<Digest, Component>>,
    app_data_dir: PathBuf,
}

impl<S: CAStorage> WasmEngine<S> {
    /// Create a new Wasm Engine manager.
    pub fn new(
        wasm_storage: S,
        app_data_dir: impl AsRef<Path>,
        lru_size: usize,
        chain_configs: ChainConfigs,
    ) -> Self {
        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);
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
            chain_configs,
        }
    }
}

impl<S: CAStorage> Engine for WasmEngine<S> {
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        // compile component (validate it is proper wasm)
        let cm = Component::new(&self.wasm_engine, bytecode)?;

        // store original wasm
        let digest = self.wasm_storage.set_data(bytecode)?;

        // // TODO: write precompiled wasm (huge optimization on restart)
        // tokio::fs::write(self.path_for_precompiled_wasm(digest), cm.serialize()?).await?;

        self.memory_cache.write().unwrap().put(digest.clone(), cm);

        Ok(digest)
    }

    // TODO: paginate this
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        let digests: Result<Vec<_>, CAStorageError> = self.wasm_storage.digests()?.collect();
        Ok(digests?)
    }

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    #[instrument(level = "debug", skip(self), fields(subsys = "Engine"))]
    fn execute(
        &self,
        wasi: &utils::types::Component,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError> {
        let (mut store, component, linker) =
            self.get_instance_deps(wasi, &trigger, service_config)?;

        store
            .set_fuel(service_config.fuel_limit)
            .map_err(EngineError::Other)?;

        self.block_on_run(async move {
            let service_id = trigger.config.service_id.clone();
            let workflow_id = trigger.config.workflow_id.clone();

            let input: crate::bindings::world::lay3r::avs::layer_types::TriggerAction = trigger
                .try_into()
                .map_err(|e: TriggerError| EngineError::TriggerData(e.into()))?;

            crate::bindings::world::LayerTriggerWorld::instantiate_async(
                &mut store, &component, &linker,
            )
            .await
            .context("Wasm instantiate failed")?
            .call_run(store, &input)
            .await
            .context("Failed to run task")
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(service_id, workflow_id),
                _ => EngineError::ComponentError(e.to_string()),
            })?
            .map_err(EngineError::ComponentError)
        })
    }
}

impl<S: CAStorage> WasmEngine<S> {
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

    fn get_instance_deps(
        &self,
        wasi: &utils::types::Component,
        trigger: &TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<(Store<HostComponent>, Component, Linker<HostComponent>), EngineError> {
        // load component from memory cache or compile from wasm
        // TODO: use serialized precompile as well, pull this into a method
        let digest = wasi.wasm.clone();
        let component = match self.memory_cache.write().unwrap().get(&digest) {
            Some(cm) => cm.clone(),
            None => {
                let bytes = self.wasm_storage.get_data(&digest)?;
                Component::new(&self.wasm_engine, &bytes)?
            }
        };

        // create linker
        let mut linker = Linker::new(&self.wasm_engine);
        crate::bindings::world::host::add_to_linker(&mut linker, |state| state).unwrap();
        // wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
        // wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).unwrap();
        wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
        // don't add http support if we don't allow it
        // FIXME: we need to apply Only(host) checks as well, but that involves some wat magic
        if wasi.permissions.allowed_http_hosts != AllowedHostPermission::None {
            wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        }

        // create wasi context
        let mut builder = WasiCtxBuilder::new();

        // conditionally allow fs access
        if wasi.permissions.file_system {
            let app_cache_path = self
                .app_data_dir
                .join(trigger.config.service_id.as_ref())
                .join(trigger.config.workflow_id.as_ref());
            if !app_cache_path.is_dir() {
                std::fs::create_dir_all(&app_cache_path)?;
            }
            builder
                .preopened_dir(&app_cache_path, ".", DirPerms::all(), FilePerms::all())
                .context("preopen failed")?;
        }

        // read in system env variables that are prefixed with WAVS_ENV and are allowed to access via the component config
        let env: Vec<_> = std::env::vars()
            .filter(|(key, _)| {
                key.starts_with("WAVS_ENV") && service_config.host_envs.contains(&key.to_string())
            })
            .chain(
                service_config
                    .kv
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone())),
            )
            .collect();

        if !env.is_empty() {
            builder.envs(&env);
        }

        let ctx = builder.build();

        // create host (what is this actually? some state needed for the linker?)
        let host = HostComponent {
            chain_configs: self.chain_configs.clone(),
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            http: WasiHttpCtx::new(),
        };

        let store = wasmtime::Store::new(&self.wasm_engine, host);

        Ok((store, component, linker))
    }
}

// TODO: revisit this an understand it.
// Copied blindly from old code
pub struct HostComponent {
    pub chain_configs: ChainConfigs,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
}

impl WasiView for HostComponent {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl WasiHttpView for HostComponent {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
}

#[cfg(test)]
mod tests {
    use utils::{
        storage::memory::MemoryStorage,
        types::{Trigger, TriggerData},
        ComponentID, ServiceID, WorkflowID,
    };

    use crate::{apis::trigger::TriggerConfig, engine::mock::mock_chain_configs};

    use super::*;

    const ECHO_RAW: &[u8] = include_bytes!("../../../../examples/build/components/echo_raw.wasm");
    const PERMISSIONS: &[u8] =
        include_bytes!("../../../../examples/build/components/permissions.wasm");

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3, ChainConfigs::default());

        // store two blobs
        let digest = engine.store_wasm(ECHO_RAW).unwrap();
        let digest2 = engine.store_wasm(PERMISSIONS).unwrap();
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
        let engine = WasmEngine::new(storage, &app_data, 3, ChainConfigs::default());

        // store valid wasm
        let digest = engine.store_wasm(ECHO_RAW).unwrap();
        // fail on invalid wasm
        engine.store_wasm(b"foobarbaz").unwrap_err();

        // only list the valid one
        let digests = engine.list_digests().unwrap();
        assert_eq!(digests, vec![digest]);
    }

    #[test]
    fn execute_echo() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3, mock_chain_configs());

        // store square digest
        let digest = engine.store_wasm(ECHO_RAW).unwrap();
        let component = utils::types::Component::new(digest);

        // execute it and get bytes back
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"{"x":12}"#),
                },
                &ServiceConfig::default(),
            )
            .unwrap();

        assert_eq!(&result, br#"{"x":12}"#);
    }

    #[test]
    fn validate_execute_config_environment() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3, mock_chain_configs());

        std::env::set_var("WAVS_ENV_TEST", "testing");
        std::env::set_var("WAVS_ENV_TEST_NOT_ALLOWED", "secret");

        let digest = engine.store_wasm(ECHO_RAW).unwrap();
        let component = utils::types::Component::new(digest);
        let service_config = ServiceConfig {
            fuel_limit: 100_000_000,
            host_envs: vec!["WAVS_ENV_TEST".to_string()],
            kv: vec![("foo".to_string(), "bar".to_string())],
            max_gas: None,
            component_id: ComponentID::default(),
            workflow_id: WorkflowID::default(),
        };

        // verify service config kv is accessible
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:foo"#),
                },
                &service_config,
            )
            .unwrap();

        assert_eq!(&result, br#"bar"#);

        // verify whitelisted host env var is accessible
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:WAVS_ENV_TEST"#),
                },
                &service_config,
            )
            .unwrap();

        assert_eq!(&result, br#"testing"#);

        // verify the non-enabled env var is not accessible
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"envvar:WAVS_ENV_TEST_NOT_ALLOWED"#),
                },
                &service_config,
            )
            .unwrap_err();
        assert!(matches!(result, EngineError::ComponentError(_)));
    }

    #[test]
    fn execute_without_enough_fuel() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();

        let low_fuel_limit = 1;
        let engine = WasmEngine::new(storage, &app_data, 3, mock_chain_configs());

        // store square digest
        let digest = engine.store_wasm(ECHO_RAW).unwrap();
        let component = utils::types::Component::new(digest);
        let service_config = ServiceConfig {
            fuel_limit: low_fuel_limit,
            ..Default::default()
        };

        // execute it and get the error
        let err = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::default(),
                        trigger: Trigger::Manual,
                    },
                    data: TriggerData::new_raw(br#"{"x":12}"#),
                },
                &service_config,
            )
            .unwrap_err();

        assert!(matches!(err, EngineError::OutOfFuel(_, _)));
    }
}
