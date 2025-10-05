use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use lru::LruCache;
use wasmtime::{component::Component as WasmComponent, Config as WTConfig, Engine as WTEngine};

use utils::config::ChainConfigs;
use utils::service::fetch_bytes;
use utils::storage::db::RedbStorage;
use utils::storage::CAStorage;
use utils::wkg::WkgClient;
use wavs_types::{ComponentDigest, ComponentSource};

use crate::utils::error::EngineError;

const DEFAULT_LRU_SIZE: usize = 10;

pub struct BaseEngineConfig {
    pub app_data_dir: PathBuf,
    pub chain_configs: Arc<RwLock<ChainConfigs>>,
    pub lru_size: usize,
    pub max_wasm_fuel: Option<u64>,
    pub max_execution_seconds: Option<u64>,
    pub ipfs_gateway: String,
}

pub struct BaseEngine<S: CAStorage> {
    pub wasm_engine: WTEngine,
    pub chain_configs: Arc<RwLock<ChainConfigs>>,
    pub memory_cache: Mutex<LruCache<ComponentDigest, WasmComponent>>,
    pub app_data_dir: PathBuf,
    pub max_wasm_fuel: Option<u64>,
    pub max_execution_seconds: Option<u64>,
    pub db: RedbStorage,
    pub storage: Arc<S>,
    pub ipfs_gateway: String,
}

impl<S: CAStorage + Send + Sync + 'static> BaseEngine<S> {
    pub fn new(
        config: BaseEngineConfig,
        db: RedbStorage,
        storage: Arc<S>,
    ) -> Result<Self, EngineError> {
        let mut wt_config = WTConfig::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);
        wt_config.epoch_interruption(true);
        let wasm_engine = WTEngine::new(&wt_config).map_err(EngineError::Compile)?;

        let lru_size = NonZeroUsize::new(config.lru_size)
            .unwrap_or(NonZeroUsize::new(DEFAULT_LRU_SIZE).unwrap());

        if !config.app_data_dir.is_dir() {
            std::fs::create_dir_all(&config.app_data_dir)
                .map_err(|e| EngineError::IO(format!("Failed to create app data dir: {}", e)))?;
        }

        // just run forever, ticking forward till the end of time (or however long this node is up)
        let engine_ticker = wasm_engine.weak();
        std::thread::spawn(move || loop {
            if let Some(engine_ticker) = engine_ticker.upgrade() {
                engine_ticker.increment_epoch();
            } else {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        });

        Ok(Self {
            wasm_engine,
            chain_configs: config.chain_configs,
            memory_cache: Mutex::new(LruCache::new(lru_size)),
            app_data_dir: config.app_data_dir,
            max_wasm_fuel: config.max_wasm_fuel,
            max_execution_seconds: config.max_execution_seconds,
            db,
            storage,
            ipfs_gateway: config.ipfs_gateway,
        })
    }

    pub async fn load_component(
        &self,
        digest: &ComponentDigest,
    ) -> Result<WasmComponent, EngineError> {
        {
            let mut cache = self.memory_cache.lock().unwrap();
            if let Some(component) = cache.get(digest) {
                return Ok(component.clone());
            }
        }

        let bytes = self
            .storage
            .get_data(&digest.clone().into())
            .map_err(|e| EngineError::StorageError(format!("Failed to get component: {}", e)))?;

        let component =
            WasmComponent::new(&self.wasm_engine, &bytes).map_err(EngineError::Compile)?;

        self.memory_cache
            .lock()
            .unwrap()
            .put(digest.clone(), component.clone());

        Ok(component)
    }

    pub async fn load_component_from_source(
        &self,
        source: &ComponentSource,
    ) -> Result<WasmComponent, EngineError> {
        let digest = source.digest();

        match self.load_component(digest).await {
            Ok(component) => Ok(component),
            Err(_) => {
                let bytes: Vec<u8> = match source {
                    ComponentSource::Download { uri, .. } => {
                        fetch_bytes(uri, &self.ipfs_gateway).await.map_err(|e| {
                            EngineError::StorageError(format!("Failed to download from url: {}", e))
                        })?
                    }
                    ComponentSource::Registry { registry } => {
                        let client =
                            WkgClient::new(registry.domain.clone().unwrap_or("wa.dev".to_string()))
                                .map_err(|e| {
                                    EngineError::StorageError(format!(
                                        "Failed to create WKG client: {}",
                                        e
                                    ))
                                })?;

                        client.fetch(registry).await.map_err(|e| {
                            EngineError::StorageError(format!(
                                "Failed to fetch from registry: {}",
                                e
                            ))
                        })?
                    }
                    _ => {
                        return Err(EngineError::UnknownDigest(digest.clone()));
                    }
                };

                if ComponentDigest::hash(&bytes) != *digest {
                    return Err(EngineError::StorageError(
                        "Downloaded component digest does not match expected digest".to_string(),
                    ));
                }

                self.storage.set_data(&bytes).map_err(|e| {
                    EngineError::StorageError(format!("Failed to store component: {}", e))
                })?;

                let component =
                    WasmComponent::new(&self.wasm_engine, &bytes).map_err(EngineError::Compile)?;

                self.memory_cache
                    .lock()
                    .unwrap()
                    .put(digest.clone(), component.clone());

                Ok(component)
            }
        }
    }

    pub fn store_component_bytes(&self, bytes: &[u8]) -> Result<ComponentDigest, EngineError> {
        // compile component (validate it is proper wasm)
        let component =
            WasmComponent::new(&self.wasm_engine, bytes).map_err(EngineError::Compile)?;

        // store original wasm
        let digest = ComponentDigest::from(
            self.storage
                .set_data(bytes)
                .map_err(|e| EngineError::StorageError(format!("Failed to store bytes: {}", e)))?
                .inner(),
        );

        // // TODO: write precompiled wasm (huge optimization on restart)
        // tokio::fs::write(self.path_for_precompiled_wasm(digest), cm.serialize()?).await?;

        self.memory_cache
            .lock()
            .unwrap()
            .put(digest.clone(), component);

        Ok(digest)
    }

    pub fn get_chain_configs(&self) -> Result<ChainConfigs, EngineError> {
        self.chain_configs
            .read()
            .map(|configs| configs.clone())
            .map_err(|e| EngineError::StorageError(format!("Chain configs lock poisoned: {}", e)))
    }
}
