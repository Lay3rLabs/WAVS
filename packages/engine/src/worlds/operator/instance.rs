use std::path::Path;

use utils::config::{ChainConfigs, WAVS_ENV_PREFIX};
use wasmtime::component::HasSelf;
use wasmtime::Store;
use wasmtime::{component::Linker, Engine as WTEngine};
use wasmtime_wasi::{p2::WasiCtxBuilder, DirPerms, FilePerms};
use wasmtime_wasi_http::WasiHttpCtx;
use wavs_types::{AllowedHostPermission, Service, Workflow, WorkflowId};

use super::component::{HostComponent, HostComponentLogger};
use crate::{backend::wasi_keyvalue::context::KeyValueCtx, utils::error::EngineError};

// how often to yield to check for epoch interruption
// this is in milliseconds since that's the unit we use for driving the epoch
// via increment_epoch()
// TODO - follow-up with questions about this... increasing to 100 breaks things
pub const EPOCH_YIELD_PERIOD_MS: u64 = 10;

pub struct InstanceDepsBuilder<'a, P> {
    pub component: wasmtime::component::Component,
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub engine: &'a WTEngine,
    pub data_dir: P,
    pub chain_configs: &'a ChainConfigs,
    pub log: HostComponentLogger,
    pub max_wasm_fuel: Option<u64>,
    pub max_execution_seconds: Option<u64>,
    pub keyvalue_ctx: KeyValueCtx,
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
                    service_id: service.id().clone(),
                    workflow_id: workflow_id.clone(),
                })?;

        let permissions = &workflow.component.permissions;

        // create linker
        let mut linker = Linker::new(engine);
        crate::bindings::operator::world::host::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |state| state,
        )
        .unwrap();
        // wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
        // wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).unwrap();
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
        // don't add http support if we don't allow it
        // FIXME: we need to apply Only(host) checks as well, but that involves some wat magic
        if permissions.allowed_http_hosts != AllowedHostPermission::None {
            wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        }

        KeyValueCtx::add_to_linker(&mut linker)?;

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
            // Users will be able to set this super high
            // but we're okay with that since wasmtime will force yield points to be interjected
            fuel_limit = fuel_limit.max(max_wasm_fuel);
        }

        let mut time_limit_seconds = workflow
            .component
            .time_limit_seconds
            .unwrap_or(Workflow::DEFAULT_TIME_LIMIT_SECONDS);

        if let Some(max_execution_seconds) = max_execution_seconds {
            time_limit_seconds = time_limit_seconds.min(max_execution_seconds);
        }

        let ctx = builder.build();

        // create host (what is this actually? some state needed for the linker?)
        let host = HostComponent {
            service,
            workflow_id,
            chain_configs: chain_configs.clone(),
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            keyvalue_ctx,
            http: WasiHttpCtx::new(),
            inner_log: log,
        };

        let mut store = wasmtime::Store::new(engine, host);

        store.set_fuel(fuel_limit).map_err(EngineError::Store)?;

        // this only configureds the component to yield periodically
        // killing is done from the outside via a tokio timeout
        store.epoch_deadline_async_yield_and_update(EPOCH_YIELD_PERIOD_MS);

        Ok(InstanceDeps {
            store,
            component,
            linker,
            time_limit_seconds,
        })
    }
}
