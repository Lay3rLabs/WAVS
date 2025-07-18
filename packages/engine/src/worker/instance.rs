use std::path::Path;

use utils::config::{ChainConfigs, WAVS_ENV_PREFIX};
use wasmtime::component::HasSelf;
use wasmtime::Store;
use wasmtime::{component::Linker, Engine as WTEngine};
use wasmtime_wasi::{p2::{WasiCtxBuilder, WasiCtx, WasiView, IoView}, DirPerms, FilePerms};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wavs_types::{AllowedHostPermission, Service, Workflow, WorkflowID};

use crate::{EngineError, keyvalue::context::KeyValueCtx};

pub struct WorkerHostComponent {
    pub service: Service,
    pub workflow_id: WorkflowID,
    pub chain_configs: ChainConfigs,
    pub inner_log: crate::HostComponentLogger,
    pub(crate) wasi_ctx: WasiCtx,
    pub(crate) keyvalue_ctx: KeyValueCtx,
    pub(crate) http_ctx: WasiHttpCtx,
    pub(crate) table: wasmtime::component::ResourceTable,
}

pub struct WorkerInstanceDepsBuilder<'a, P> {
    pub component: wasmtime::component::Component,
    pub service: Service,
    pub workflow_id: WorkflowID,
    pub engine: &'a WTEngine,
    pub data_dir: P,
    pub chain_configs: &'a ChainConfigs,
    pub log: crate::HostComponentLogger,
    pub max_wasm_fuel: Option<u64>,
    pub max_execution_seconds: Option<u64>,
    pub keyvalue_ctx: KeyValueCtx,
}

pub struct WorkerInstanceDeps {
    pub store: Store<WorkerHostComponent>,
    pub component: wasmtime::component::Component,
    pub linker: Linker<WorkerHostComponent>,
    pub time_limit_seconds: u64,
}

impl<P: AsRef<Path>> WorkerInstanceDepsBuilder<'_, P> {
    pub fn build(self) -> Result<WorkerInstanceDeps, EngineError> {
        let Self {
            component,
            service,
            workflow_id,
            engine,
            data_dir,
            chain_configs,
            log,
            keyvalue_ctx,
            max_execution_seconds,
            max_wasm_fuel,
        } = self;

        let workflow =
            service
                .workflows
                .get(&workflow_id)
                .ok_or_else(|| EngineError::WorkflowNotFound {
                    workflow_id: workflow_id.clone(),
                    service_id: service.id().clone(),
                })?;

        let mut linker = Linker::<WorkerHostComponent>::new(engine);
        super::bindings::world::host::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .unwrap();
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
        if workflow.component.permissions.allowed_http_hosts != AllowedHostPermission::None {
            wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        }

        KeyValueCtx::add_to_linker(&mut linker)?;

        let wasi_ctx = create_wasi_ctx(&workflow, &data_dir)?;
        let keyvalue_ctx = keyvalue_ctx.clone();
        let http_ctx = WasiHttpCtx::new();

        let host_component = WorkerHostComponent {
            service,
            workflow_id,
            chain_configs: chain_configs.clone(),
            inner_log: log,
            wasi_ctx,
            keyvalue_ctx,
            http_ctx,
            table: wasmtime::component::ResourceTable::new(),
        };

        let mut store = Store::new(engine, host_component);

        if let Some(fuel) = max_wasm_fuel {
            store.set_fuel(fuel).map_err(EngineError::Store)?;
        }

        Ok(WorkerInstanceDeps {
            store,
            component,
            linker,
            time_limit_seconds: max_execution_seconds.unwrap_or(60),
        })
    }
}

fn create_wasi_ctx<P: AsRef<Path>>(
    workflow: &Workflow,
    data_dir: P,
) -> Result<WasiCtx, EngineError> {
    let mut binding = WasiCtxBuilder::new();
    let mut wasi_ctx = binding
        .inherit_stdio()
        .inherit_stdout()
        .inherit_stderr();

    if workflow.component.permissions.file_system {
        wasi_ctx =
            wasi_ctx.preopened_dir(data_dir.as_ref(), "/", DirPerms::all(), FilePerms::all())
                .map_err(EngineError::Filesystem)?;
    }

    let env: Vec<_> = std::env::vars()
        .filter(|(key, _)| {
            key.starts_with(WAVS_ENV_PREFIX) && workflow.component.env_keys.contains(key)
        })
        .collect();

    for (key, value) in env {
        wasi_ctx = wasi_ctx.env(&key, &value);
    }

    Ok(wasi_ctx.build())
}

impl WasiView for WorkerHostComponent {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

impl IoView for WorkerHostComponent {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for WorkerHostComponent {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http_ctx
    }
}
