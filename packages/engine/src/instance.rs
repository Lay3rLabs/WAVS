use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use utils::config::{ChainConfigs, WAVS_ENV_PREFIX};
use wasmtime::Store;
use wasmtime::{component::Linker, Engine as WTEngine};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};
use wasmtime_wasi_http::WasiHttpCtx;
use wasmtime_wasi_keyvalue::{WasiKeyValue, WasiKeyValueCtx};
use wavs_types::{AllowedHostPermission, ServiceID, Workflow, WorkflowID};

use crate::{EngineError, HostComponent, HostComponentLogger};

pub struct InstanceDepsBuilder<'a, P> {
    pub component: wasmtime::component::Component,
    pub workflow: Workflow,
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub engine: &'a WTEngine,
    pub data_dir: P,
    pub chain_configs: &'a ChainConfigs,
    pub log: HostComponentLogger,
    pub max_wasm_fuel: Option<u64>,
    pub max_execution_seconds: Option<u64>,
    pub shared_keyvalue_store: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

pub struct InstanceDeps {
    pub store: Store<HostComponent>,
    pub component: wasmtime::component::Component,
    pub linker: Linker<HostComponent>,
    pub time_limit_seconds: u64,
}

impl<P: AsRef<Path>> InstanceDepsBuilder<'_, P> {
    pub fn build(self) -> Result<InstanceDeps, EngineError> {
        let Self {
            component,
            workflow,
            service_id,
            workflow_id,
            engine,
            data_dir,
            chain_configs,
            log,
            max_execution_seconds,
            max_wasm_fuel,
            shared_keyvalue_store,
        } = self;

        let permissions = &workflow.component.permissions;

        // create linker
        let mut linker = Linker::new(engine);
        crate::bindings::world::host::add_to_linker(&mut linker, |state| state).unwrap();
        // wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
        // wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).unwrap();
        wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
        // don't add http support if we don't allow it
        // FIXME: we need to apply Only(host) checks as well, but that involves some wat magic
        if permissions.allowed_http_hosts != AllowedHostPermission::None {
            wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        }
        wasmtime_wasi_keyvalue::add_to_linker(&mut linker, |h: &mut HostComponent| {
            WasiKeyValue::new(&h.keyvalue, &mut h.table)
        })
        .unwrap();

        // create wasi context
        let mut builder = WasiCtxBuilder::new();

        builder.inherit_stdout().inherit_stderr();

        // conditionally allow fs access
        if permissions.file_system {
            // we namespace by service id so that all components within a service have access to the same data
            // and services are each isolated from each other
            let data_dir = data_dir.as_ref();
            if !data_dir.is_dir() {
                std::fs::create_dir_all(data_dir).map_err(|e| EngineError::Filesystem(e.into()))?;
            }
            builder
                .preopened_dir(data_dir, ".", DirPerms::all(), FilePerms::all())
                .map_err(EngineError::Filesystem)?;
        }

        // read in system env variables that are prefixed with WAVS_ENV and are allowed to access via the component config
        let env: Vec<_> = std::env::vars()
            .filter(|(key, _)| {
                key.starts_with(WAVS_ENV_PREFIX) && workflow.component.env_keys.contains(key)
            })
            .collect();

        if !env.is_empty() {
            builder.envs(&env);
        }

        let mut fuel_limit = workflow
            .component
            .fuel_limit
            .unwrap_or(Workflow::DEFAULT_FUEL_LIMIT);

        if let Some(max_wasm_fuel) = max_wasm_fuel {
            fuel_limit = fuel_limit.min(max_wasm_fuel);
        }

        let mut time_limit_seconds = workflow
            .component
            .time_limit_seconds
            .unwrap_or(Workflow::DEFAULT_TIME_LIMIT_SECONDS);

        if let Some(max_execution_seconds) = max_execution_seconds {
            time_limit_seconds = time_limit_seconds.min(max_execution_seconds);
        }

        let ctx = builder.build();

        // create keyvalue context - will be replaced with shared store in HostComponent
        let keyvalue = {
            use wasmtime_wasi_keyvalue::WasiKeyValueCtxBuilder;
            WasiKeyValueCtxBuilder::new().build()
        };

        // create host (what is this actually? some state needed for the linker?)
        let host = HostComponent {
            workflow,
            service_id,
            workflow_id,
            chain_configs: chain_configs.clone(),
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            http: WasiHttpCtx::new(),
            keyvalue,
            shared_keyvalue_store,
            inner_log: log,
        };

        let mut store = wasmtime::Store::new(engine, host);

        store.set_fuel(fuel_limit).map_err(EngineError::Store)?;

        // This time limit kills things from _within_ the Wasm instance
        // and is not the same as the time limit from the host side, which still needs to be imposed
        // see https://github.com/bytecodealliance/wasmtime-go/issues/233#issuecomment-2356238658
        store.set_epoch_deadline(time_limit_seconds);

        Ok(InstanceDeps {
            store,
            component,
            linker,
            time_limit_seconds,
        })
    }
}
