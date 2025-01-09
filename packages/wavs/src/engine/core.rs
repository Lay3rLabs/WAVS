use anyhow::Context;
use layer_climb::prelude::Address;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::instrument;
use wasmtime::{
    component::{Component, Linker},
    Config as WTConfig, Engine as WTEngine,
};
use wasmtime::{Store, Trap};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::apis::dispatcher::{AllowedHostPermission, ComponentWorld, ServiceConfig};
use crate::apis::trigger::{TriggerAction, TriggerData};
use crate::storage::{CAStorage, CAStorageError};
use crate::{apis, bindings, Digest};
use utils::{ServiceID, WorkflowID};

use super::{Engine, EngineError};

pub struct WasmEngine<S: CAStorage> {
    wasm_storage: S,
    wasm_engine: WTEngine,
    memory_cache: RwLock<LruCache<Digest, Component>>,
    app_data_dir: PathBuf,
}

impl<S: CAStorage> WasmEngine<S> {
    /// Create a new Wasm Engine manager.
    pub fn new(wasm_storage: S, app_data_dir: impl AsRef<Path>, lru_size: usize) -> Self {
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
        wasi: &apis::dispatcher::Component,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError> {
        let world = wasi.world;
        let (mut store, component, linker) = self.get_instance_deps(wasi, &trigger, service_config)?;

        store
            .set_fuel(service_config.fuel_limit)
            .map_err(EngineError::Other)?;

        self.block_on_run(async move {
            let contract = match &trigger.data {
                TriggerData::CosmosContractEvent {
                    contract_address, ..
                } => Some(contract_address.clone()),
                TriggerData::EthContractEvent {
                    contract_address, ..
                } => Some(contract_address.clone()),
                TriggerData::Raw(_) => None,
            };

            let chain_id = match &trigger.data {
                TriggerData::CosmosContractEvent { chain_id, .. } => Some(chain_id.clone()),
                TriggerData::EthContractEvent { chain_id, .. } => Some(chain_id.clone()),
                TriggerData::Raw(_) => None,
            };

            let res = match world {
                ComponentWorld::ChainEvent => {
                    let contract = match (contract, chain_id) {
                        (Some(contract), Some(chain_id)) => bindings::chain_event::Contract {
                            address: match contract {
                                Address::Cosmos {
                                    bech32_addr,
                                    prefix_len,
                                } => {
                                    bindings::chain_event::lay3r::avs::wavs_types::Address::Cosmos(
                                        (bech32_addr, prefix_len.try_into().unwrap()),
                                    )
                                }
                                Address::Eth(addr) => {
                                    bindings::chain_event::lay3r::avs::wavs_types::Address::Eth(
                                        addr.as_bytes().to_vec(),
                                    )
                                }
                            },
                            chain_id,
                        },
                        _ => {
                            return Err(EngineError::ComponentError(
                                "No contract address provided".to_string(),
                            ));
                        }
                    };

                    let data = match trigger.data.into_vec() {
                        Some(data) => data,
                        None => {
                            return Err(EngineError::ComponentError(
                                "No data provided".to_string(),
                            ));
                        }
                    };

                    bindings::chain_event::WavsChainEventWorld::instantiate_async(
                        &mut store, &component, &linker,
                    )
                    .await
                    .context("Wasm instantiate failed")?
                    .call_run(store, &contract, &data)
                    .await
                }
                ComponentWorld::EthLog => {
                    let eth_log = match trigger.data {
                        TriggerData::EthContractEvent { log, .. } => {
                            bindings::eth_log::lay3r::avs::wavs_types::EthLog {
                                topics: log.topics().iter().map(|t| t.to_vec()).collect(),
                                data: log.data.to_vec(),
                            }
                        }
                        _ => {
                            return Err(EngineError::ComponentError("No log provided".to_string()));
                        }
                    };
                    let contract = match (contract, chain_id) {
                        (Some(contract), Some(chain_id)) => bindings::eth_log::Contract {
                            address: match contract {
                                Address::Cosmos {
                                    bech32_addr,
                                    prefix_len,
                                } => bindings::eth_log::lay3r::avs::wavs_types::Address::Cosmos((
                                    bech32_addr,
                                    prefix_len.try_into().unwrap(),
                                )),
                                Address::Eth(addr) => {
                                    bindings::eth_log::lay3r::avs::wavs_types::Address::Eth(
                                        addr.as_bytes().to_vec(),
                                    )
                                }
                            },
                            chain_id,
                        },
                        _ => {
                            return Err(EngineError::ComponentError(
                                "No contract address provided".to_string(),
                            ));
                        }
                    };

                    bindings::eth_log::WavsEthLogWorld::instantiate_async(
                        &mut store, &component, &linker,
                    )
                    .await
                    .context("Wasm instantiate failed")?
                    .call_run(store, &contract, &eth_log)
                    .await
                }
                ComponentWorld::Raw => {
                    let data = match trigger.data.into_vec() {
                        Some(data) => data,
                        None => {
                            return Err(EngineError::ComponentError(
                                "No data provided".to_string(),
                            ));
                        }
                    };

                    bindings::raw::WavsRawWorld::instantiate_async(&mut store, &component, &linker)
                        .await
                        .context("Wasm instantiate failed")?
                        .call_run(store, &data)
                        .await
                }
            };

            res.context("Failed to run task")
                .map_err(|e| match e.downcast_ref::<Trap>() {
                    Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(
                        trigger.config.service_id,
                        trigger.config.workflow_id,
                    ),
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

    fn get_instance_deps<L: WasiView + WasiHttpView>(
        &self,
        wasi: &apis::dispatcher::Component,
        trigger: &TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<(Store<Host>, Component, Linker<L>), EngineError> {
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
        let host = Host {
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
pub(crate) struct Host {
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
}

impl WasiView for Host {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl WasiHttpView for Host {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
}

#[cfg(test)]
mod tests {
    use apis::{
        trigger::{Trigger, TriggerConfig},
        dispatcher::ServiceConfig,
        ServiceID, WorkflowID,
    };

    use crate::{storage::memory::MemoryStorage, test_utils::address::rand_address_layer};

    use super::*;

    const SQUARE: &[u8] = include_bytes!("../../../../examples/build/components/square.wasm");
    const PERMISSIONS: &[u8] =
        include_bytes!("../../../../examples/build/components/permissions.wasm");
    const DEFAULT_FUEL_LIMIT: u64 = 1_000_000;

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3);

        // store two blobs
        let digest = engine.store_wasm(SQUARE).unwrap();
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
        let engine = WasmEngine::new(storage, &app_data, 3);

        // store valid wasm
        let digest = engine.store_wasm(SQUARE).unwrap();
        // fail on invalid wasm
        engine.store_wasm(b"foobarbaz").unwrap_err();

        // only list the valid one
        let digests = engine.list_digests().unwrap();
        assert_eq!(digests, vec![digest]);
    }

    #[test]
    fn execute_square() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3);

        // store square digest
        let digest = engine.store_wasm(SQUARE).unwrap();
        let component = crate::apis::dispatcher::Component::new(digest, ComponentWorld::ChainEvent);

        // execute it and get square
        let result = engine
            .execute(
                &component,
                TriggerAction {
                    config: TriggerConfig {
                        service_id: ServiceID::new("foobar").unwrap(),
                        workflow_id: WorkflowID::new("default").unwrap(),
                        trigger: Trigger::Test,
                    },
                    data: TriggerData::CosmosContractEvent {
                        contract_address: rand_address_layer(),
                        chain_id: "cosmos".to_string(),
                        event_data: Some(br#"{"x":12}"#.to_vec()),
                    },
                },
                &ServiceConfig::default()
            )
            .unwrap();
        assert_eq!(&result, br#"{"y":144}"#);
    }

    #[test]
    fn validate_execute_config_environment() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3);

        std::env::set_var("WAVS_ENV_TEST", "testing");
        std::env::set_var("WAVS_ENV_TEST_NOT_ALLOWED", "secret");

        let digest = engine.store_wasm(ETH_TRIGGER_ECHO).unwrap();
        let component = crate::apis::dispatcher::Component::new(&digest, ComponentWorld::ChainEvent);
        let service_config = ServiceConfig {
            fuel_limit: 100_000_000,
            host_envs: vec!["WAVS_ENV_TEST".to_string()],
            kv: vec![("foo".to_string(), "bar".to_string())],
            max_gas: None,
            component_id: ComponentID::default(),
            workflow_id: workflow_id.clone(),
        };

        // verify service config kv is accessible
        let result = engine
            .execute_eth_event(
                &component,
                &service_config,
                &ServiceID::new("foobar").unwrap(),
                &workflow_id,
                TriggerId::new(12345),
                br#"envvar:foo"#.into(),
            )
            .unwrap();
        assert_eq!(&result, br#"bar"#);

        // verify whitelisted host env var is accessible
        let result = engine
            .execute_eth_event(
                &component,
                &service_config,
                &ServiceID::new("foobar").unwrap(),
                &workflow_id,
                TriggerId::new(12345),
                br#"envvar:WAVS_ENV_TEST"#.into(),
            )
            .unwrap();
        assert_eq!(&result, br#"testing"#);

        // verify the non-enabled env var is not accessible
        let result = engine
            .execute_eth_event(
                &component,
                &service_config,
                &ServiceID::new("foobar").unwrap(),
                &workflow_id,
                TriggerId::new(12345),
                br#"envvar:WAVS_ENV_TEST_NOT_ALLOWED"#.into(),
            )
            .unwrap_err();
        assert!(matches!(result, EngineError::ComponentError(_)));
    }

    #[test]
    fn execute_without_enough_fuel() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();

        let low_fuel_limit = 1;
        let engine = WasmEngine::new(storage, &app_data, 3);

        // store square digest
        let digest = engine.store_wasm(SQUARE).unwrap();
        let component = crate::apis::dispatcher::Component::new(digest, ComponentWorld::ChainEvent);
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
                        workflow_id: WorkflowID::new("default").unwrap(),
                        trigger: Trigger::Test,
                    },
                    data: TriggerData::CosmosContractEvent {
                        contract_address: rand_address_layer(),
                        chain_id: "cosmos".to_string(),
                        event_data: Some(br#"{"x":12}"#.to_vec()),
                    },
                },
                &service_config
            )
            .unwrap_err();

        assert!(matches!(err, EngineError::OutOfFuel(_, _)));
    }
}
