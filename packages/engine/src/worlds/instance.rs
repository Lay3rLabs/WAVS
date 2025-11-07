use std::path::Path;

use utils::config::WAVS_ENV_PREFIX;
use wasmtime::component::HasSelf;
use wasmtime::Store;
use wasmtime::{component::Linker, Engine as WTEngine};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wavs_types::{AllowedHostPermission, ChainConfigs, Permissions, Service, Workflow, WorkflowId};

use crate::backend::wasi_keyvalue::context::KeyValueCtxProvider;
use crate::worlds::aggregator::component::{
    AggregatorHostComponent, AggregatorHostComponentLogger,
};
use crate::worlds::operator::component::{OperatorHostComponent, OperatorHostComponentLogger};
use crate::{backend::wasi_keyvalue::context::KeyValueCtx, utils::error::EngineError};

// how often to yield to check for epoch interruption
// this is in milliseconds since that's the unit we use for driving the epoch
// via increment_epoch()
pub const EPOCH_YIELD_PERIOD_MS: u64 = 100;

pub enum HostComponentLogger {
    OperatorHostComponentLogger(OperatorHostComponentLogger),
    AggregatorHostComponentLogger(AggregatorHostComponentLogger),
}

pub enum ComponentStore {
    OperatorComponentStore(Store<OperatorHostComponent>),
    AggregatorComponentStore(Store<AggregatorHostComponent>),
}

impl ComponentStore {
    pub fn get_fuel(&self) -> anyhow::Result<u64> {
        match self {
            ComponentStore::OperatorComponentStore(store) => store.get_fuel(),
            ComponentStore::AggregatorComponentStore(store) => store.get_fuel(),
        }
    }

    pub fn as_operator_mut(&mut self) -> &mut Store<OperatorHostComponent> {
        match self {
            ComponentStore::OperatorComponentStore(store) => store,
            _ => unreachable!(),
        }
    }

    pub fn as_aggregator_mut(&mut self) -> &mut Store<AggregatorHostComponent> {
        match self {
            ComponentStore::AggregatorComponentStore(store) => store,
            _ => unreachable!(),
        }
    }
}

pub enum ComponentLinker {
    OperatorComponentLinker(Linker<OperatorHostComponent>),
    AggregatorComponentLinker(Linker<AggregatorHostComponent>),
}

impl ComponentLinker {
    pub fn as_operator_ref(&self) -> &Linker<OperatorHostComponent> {
        match self {
            ComponentLinker::OperatorComponentLinker(linker) => linker,
            _ => unreachable!(),
        }
    }
    pub fn as_aggregator_ref(&self) -> &Linker<AggregatorHostComponent> {
        match self {
            ComponentLinker::AggregatorComponentLinker(linker) => linker,
            _ => unreachable!(),
        }
    }
}
pub struct InstanceDepsBuilder<'a, P> {
    pub component: wasmtime::component::Component,
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub event_id: wavs_types::EventId,
    pub engine: &'a WTEngine,
    pub data_dir: P,
    pub chain_configs: &'a ChainConfigs,
    pub log: HostComponentLogger,
    pub keyvalue_ctx: KeyValueCtx,
}

pub struct InstanceDeps {
    pub store: ComponentStore,
    pub component: wasmtime::component::Component,
    pub linker: ComponentLinker,
    pub time_limit_seconds: u64,
}

impl<P: AsRef<Path>> InstanceDepsBuilder<'_, P> {
    pub fn build(self) -> Result<InstanceDeps, EngineError> {
        let Self {
            component,
            service,
            workflow_id,
            event_id,
            engine,
            data_dir,
            chain_configs,
            log,
            keyvalue_ctx,
        } = self;

        // create linker
        let (linker, wavs_component) = {
            let workflow = service.workflows.get(&workflow_id).ok_or_else(|| {
                EngineError::WorkflowNotFound {
                    service_id: service.id().clone(),
                    workflow_id: workflow_id.clone(),
                }
            })?;
            match log {
                HostComponentLogger::OperatorHostComponentLogger(_) => {
                    let mut linker = Linker::new(engine);

                    crate::bindings::operator::world::host::add_to_linker::<_, HasSelf<_>>(
                        &mut linker,
                        |state| state,
                    )
                    .unwrap();

                    let component = workflow.component.clone();
                    configure_linker(&mut linker, &component.permissions)?;

                    (ComponentLinker::OperatorComponentLinker(linker), component)
                }
                HostComponentLogger::AggregatorHostComponentLogger(_) => {
                    let mut linker = Linker::new(engine);

                    crate::bindings::aggregator::world::host::add_to_linker::<_, HasSelf<_>>(
                        &mut linker,
                        |state| state,
                    )
                    .unwrap();

                    let component = match &workflow.submit {
                        wavs_types::Submit::None => unreachable!(),
                        wavs_types::Submit::Aggregator { component, .. } => (**component).clone(),
                    };
                    configure_linker(&mut linker, &component.permissions)?;

                    (
                        ComponentLinker::AggregatorComponentLinker(linker),
                        component,
                    )
                }
            }
        };

        // create wasi context
        let mut builder = WasiCtxBuilder::new();

        builder.inherit_stdout().inherit_stderr();

        // conditionally allow fs access
        if wavs_component.permissions.file_system {
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

        // conditionally allow raw network access
        if wavs_component.permissions.raw_sockets {
            builder.inherit_network();
        }

        // conditionally allow dns resolution
        if wavs_component.permissions.dns_resolution {
            builder.allow_ip_name_lookup(true);
        }

        // read in system env variables that are prefixed with WAVS_ENV and are allowed to access via the component config
        let env: Vec<_> = std::env::vars()
            .filter(|(key, _)| {
                key.starts_with(WAVS_ENV_PREFIX) && wavs_component.env_keys.contains(key)
            })
            .collect();

        if !env.is_empty() {
            builder.envs(&env);
        }

        let fuel_limit = wavs_component
            .fuel_limit
            .unwrap_or(Workflow::DEFAULT_FUEL_LIMIT);

        let time_limit_seconds = wavs_component
            .time_limit_seconds
            .unwrap_or(Workflow::DEFAULT_TIME_LIMIT_SECONDS);

        let ctx = builder.build();

        // create host (what is this actually? some state needed for the linker?)
        let store = match log {
            HostComponentLogger::OperatorHostComponentLogger(log) => {
                let host = OperatorHostComponent {
                    service,
                    workflow_id,
                    event_id,
                    chain_configs: chain_configs.clone(),
                    table: wasmtime::component::ResourceTable::new(),
                    ctx,
                    keyvalue_ctx,
                    http: WasiHttpCtx::new(),
                    inner_log: log,
                };
                let mut store = wasmtime::Store::new(engine, host);

                configure_store(&mut store, fuel_limit)?;

                ComponentStore::OperatorComponentStore(store)
            }
            HostComponentLogger::AggregatorHostComponentLogger(log) => {
                let host = AggregatorHostComponent {
                    service,
                    workflow_id,
                    event_id,
                    chain_configs: chain_configs.clone(),
                    table: wasmtime::component::ResourceTable::new(),
                    ctx,
                    keyvalue_ctx,
                    http: WasiHttpCtx::new(),
                    inner_log: log,
                };
                let mut store = wasmtime::Store::new(engine, host);

                configure_store(&mut store, fuel_limit)?;

                ComponentStore::AggregatorComponentStore(store)
            }
        };

        Ok(InstanceDeps {
            store,
            component,
            linker,
            time_limit_seconds,
        })
    }
}

fn configure_store<T>(store: &mut Store<T>, fuel_limit: u64) -> Result<(), EngineError> {
    store.set_fuel(fuel_limit).map_err(EngineError::Store)?;

    // this only configures the component to yield periodically
    // killing is done from the outside via a tokio timeout
    // The reason we use epoch_deadline_callback instead of epoch_deadline_async_yield_and_update
    // is because the latter appears to have a bug where it doesn't always schedule nicely with tokio
    // See https://github.com/dakom/debug-wasmtime-concurrency for more info
    store.epoch_deadline_callback(move |_| {
        Ok(wasmtime::UpdateDeadline::YieldCustom(
            EPOCH_YIELD_PERIOD_MS,
            Box::pin(tokio::task::yield_now()),
        ))
    });

    Ok(())
}

fn configure_linker<T: WasiView + WasiHttpView + KeyValueCtxProvider>(
    linker: &mut Linker<T>,
    permissions: &Permissions,
) -> Result<(), EngineError> {
    // wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
    // wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).unwrap();
    wasmtime_wasi::p2::add_to_linker_async(linker).unwrap();
    // don't add http support if we don't allow it
    // FIXME: we need to apply Only(host) checks as well, but that involves some wat magic
    if permissions.allowed_http_hosts != AllowedHostPermission::None {
        wasmtime_wasi_http::add_only_http_to_linker_async(linker).unwrap();
    }

    KeyValueCtx::add_to_linker(linker)?;

    Ok(())
}
