use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use anyhow::Context;
use lru::LruCache;
use tracing::instrument;
use wasmtime::{
    component::{Component, Linker},
    Config as WTConfig, Engine as WTEngine,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::apis::dispatcher::AllowedHostPermission;
use crate::storage::{CAStorage, CAStorageError};
use crate::{apis, task_bindings, Digest};

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
    fn execute_queue(
        &self,
        wasi: &apis::dispatcher::Component,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
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
            let app_cache_path = self.app_data_dir.join(digest.to_string());
            if !app_cache_path.is_dir() {
                std::fs::create_dir(&app_cache_path)?;
            }
            builder
                .preopened_dir(&app_cache_path, ".", DirPerms::all(), FilePerms::all())
                .context("preopen failed")?;
        }

        // add any env vars that were provided
        if !wasi.env.is_empty() {
            builder.envs(&wasi.env);
        }
        let ctx = builder.build();

        // create host (what is this actually? some state needed for the linker?)
        let host = Host {
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            http: WasiHttpCtx::new(),
        };
        let mut store = wasmtime::Store::new(&self.wasm_engine, host);

        // This feels like an ugly hack.
        // I cannot figure out how to add wasi http support and keep this sync
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on({
            async {
                let bindings =
                    task_bindings::TaskQueue::instantiate_async(&mut store, &component, &linker)
                        .await
                        .context("Wasm instantiate failed")?;
                let input = task_bindings::lay3r::avs::types::TaskQueueInput { timestamp, request };
                let result = bindings
                    .call_run_task(&mut store, &input)
                    .await
                    .context("Failed to run task")?
                    .map_err(EngineError::ComponentError)?;
                Ok::<Vec<u8>, EngineError>(result)
            }
        })
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
    use crate::storage::memory::MemoryStorage;

    use super::*;

    const SQUARE: &[u8] = include_bytes!("../../components/square.wasm");
    const BTC_AVG: &[u8] = include_bytes!("../../components/btc_avg.wasm");

    #[test]
    fn store_and_list_wasm() {
        let storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let engine = WasmEngine::new(storage, &app_data, 3);

        // store two blobs
        let digest = engine.store_wasm(SQUARE).unwrap();
        let digest2 = engine.store_wasm(BTC_AVG).unwrap();
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
        let component = crate::apis::dispatcher::Component::new(&digest);

        // execute it and get square
        let result = engine
            .execute_queue(&component, br#"{"x":12}"#.into(), 12345)
            .unwrap();
        assert_eq!(&result, br#"{"y":144}"#);
    }
}
