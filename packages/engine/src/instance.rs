use std::path::Path;

use utils::config::ChainConfigs;
use wasmtime::Store;
use wasmtime::{
    component::{Component, Linker},
    Engine as WTEngine,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};
use wasmtime_wasi_http::WasiHttpCtx;
use wavs_types::{
    AllowedHostPermission, Digest, Permissions, ServiceConfig, ServiceID, Workflow, WorkflowID,
};

use crate::{EngineError, HostComponent, HostComponentLogger};

pub struct InstanceDepsBuilder<'a, P> {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub digest: Digest,
    pub component: Component,
    pub engine: &'a WTEngine,
    pub permissions: &'a Permissions,
    pub data_dir: P,
    pub service_config: &'a ServiceConfig,
    // will use Workflow::DEFAULT_FUEL_LIMIT if None
    pub fuel_limit: Option<u64>,
    pub chain_configs: &'a ChainConfigs,
    pub log: HostComponentLogger,
}

pub struct InstanceDeps {
    pub store: Store<HostComponent>,
    pub component: Component,
    pub linker: Linker<HostComponent>,
}

impl<P: AsRef<Path>> InstanceDepsBuilder<'_, P> {
    pub fn build(self) -> Result<InstanceDeps, EngineError> {
        let Self {
            service_id,
            workflow_id,
            digest,
            component,
            engine,
            permissions,
            data_dir,
            service_config,
            fuel_limit,
            chain_configs,
            log,
        } = self;

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
            service_id,
            workflow_id,
            digest,
            chain_configs: chain_configs.clone(),
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            http: WasiHttpCtx::new(),
            inner_log: log,
        };

        let mut store = wasmtime::Store::new(engine, host);

        store
            .set_fuel(fuel_limit.unwrap_or(Workflow::DEFAULT_FUEL_LIMIT))
            .map_err(EngineError::Store)?;

        Ok(InstanceDeps {
            store,
            component,
            linker,
        })
    }
}
