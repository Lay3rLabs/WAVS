use anyhow::{Context, Result};
use axum::{
    routing::{delete, get, post},
    Router,
};
use cw_orch::prelude::Addr;
use indexmap::IndexMap;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_cron_scheduler::{Job, JobScheduler};
use wasmtime::{
    component::{bindgen, Linker},
    Config, Engine,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::queue::QueueExecutor;
use crate::storage::{FileSystemStorage, Storage};
use crate::{app, queue::AppData};
mod add_application;
mod delete_application;
mod list_applications;
//mod update_application;

bindgen!({
    async: true,
    with: {
        "wasi": wasmtime_wasi::bindings,
        "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
    },
});

enum AppTrigger {
    Cron(uuid::Uuid),
    Queue(JoinHandle<()>),
    _Event,
}
pub struct Operator<S>
where
    S: Storage,
{
    engine: Engine,
    scheduler: JobScheduler,
    queue_executor: QueueExecutor,
    active_apps: IndexMap<String, AppTrigger>,
    envs: Vec<(String, String)>,
    storage: S,
}

impl<S: Storage + 'static> Operator<S> {
    pub async fn new(storage: S, envs: Vec<(String, String)>) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();

        let scheduler = JobScheduler::new().await?;
        scheduler.start().await?;

        let active_apps = IndexMap::new();

        let mut operator = Operator {
            engine,
            scheduler,
            queue_executor: QueueExecutor::new(),
            active_apps,
            envs,
            storage,
        };
        for app_name in operator.storage.list_application_names().await? {
            operator.activate_app(&app_name).await?;
        }

        Ok(operator)
    }

    pub async fn serve(self, bind_addr: SocketAddr) -> Result<()> {
        let router = Router::new()
            .route("/app", get(list_applications::list))
            .route("/app", post(add_application::add))
            //.route("/app", put(update_application::update))
            .route("/app", delete(delete_application::delete))
            .with_state(Arc::new(Mutex::new(self)));

        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("failed to bind to address `{bind_addr}`"))?;

        println!("Listening on {}", bind_addr);

        axum::serve::serve(listener, router.into_make_service()).await?;
        Ok(())
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    pub async fn activate_app(&mut self, name: &str) -> Result<()> {
        if self.active_apps.contains_key(name) {
            return Err(anyhow::anyhow!("app already scheduled and active"));
        }
        let app = self
            .storage
            .get_application(name)
            .await?
            .ok_or(anyhow::anyhow!("app not found"))?;

        match app.trigger {
            app::Trigger::Cron { schedule } => {
                let component = self.storage.get_wasm(&app.digest, &self.engine).await?;

                let mut linker = Linker::new(self.engine());
                wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
                wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();

                // setup app cache directory
                let app_cache_path = self.storage.path_for_app_cache(name);
                if !app_cache_path.is_dir() {
                    tokio::fs::create_dir(&app_cache_path).await?;
                }

                let mut envs = self.envs.clone();
                envs.extend_from_slice(&app.envs);

                let engine = self.engine.clone();

                let id = self
                    .scheduler
                    .add(Job::new_async(schedule.as_str(), move |_, _| {
                        Box::pin({
                            let envs = envs.clone();
                            let app_cache_path = app_cache_path.clone();
                            let engine = engine.clone();
                            let component = component.clone();
                            let linker = linker.clone();

                            async move {
                                let mut builder = WasiCtxBuilder::new();
                                if !envs.is_empty() {
                                    builder.envs(&envs);
                                }
                                builder
                                    .preopened_dir(
                                        app_cache_path,
                                        ".",
                                        DirPerms::all(),
                                        FilePerms::all(),
                                    )
                                    .expect("preopen failed");
                                let ctx = builder.build();

                                let host = Host {
                                    table: wasmtime::component::ResourceTable::new(),
                                    ctx,
                                    http: WasiHttpCtx::new(),
                                };
                                let mut store = wasmtime::Store::new(&engine, host);

                                let bindings =
                                    TaskQueue::instantiate_async(&mut store, &component, &linker)
                                        .await
                                        .expect("Wasm instantiate failed");

                                let input = lay3r::avs::task_queue_types::Input {
                                    timestamp: SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .expect("failed to get current time")
                                        .as_secs(),
                                    request: "".to_string(),
                                };

                                let output = bindings
                                    .call_run_task(&mut store, &input)
                                    .await
                                    .expect("Wasm panic");

                                dbg!(output);
                            }
                        })
                    })?)
                    .await?;

                // TODO handle possible race condition on adding job

                // save the job ID in the active CRON jobs
                self.active_apps
                    .insert(name.to_string(), AppTrigger::Cron(id));
            }
            app::Trigger::Queue {
                task_queue_addr,
                hd_index,
                poll_interval,
            } => {
                let component = self.storage.get_wasm(&app.digest, &self.engine).await?;
                let lay3r = self
                    .queue_executor
                    .builder
                    .hd_index(hd_index)
                    .build()
                    .await?;

                let app_cache_path = self.storage.path_for_app_cache(name);
                if !app_cache_path.is_dir() {
                    tokio::fs::create_dir(&app_cache_path).await?;
                }
                let mut linker = Linker::new(self.engine());
                wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
                wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();

                let mut envs = self.envs.clone();
                envs.extend_from_slice(&app.envs);
                let mut builder = WasiCtxBuilder::new();
                builder
                    .preopened_dir(app_cache_path, ".", DirPerms::all(), FilePerms::all())
                    .expect("preopen failed");
                let ctx = builder.build();
                let host = Host {
                    table: wasmtime::component::ResourceTable::new(),
                    ctx,
                    http: WasiHttpCtx::new(),
                };
                let store = wasmtime::Store::new(&self.engine(), host);

                let query = lch_apis::tasks::CustomQueryMsg::Config {};
                let config: lch_apis::tasks::ConfigResponse =
                    lay3r.query(&query, &task_queue_addr).await.unwrap();
                let handle = self.queue_executor.add_app(
                    name.to_string(),
                    AppData {
                        task_queue_addr: task_queue_addr.clone(),
                        lay3r,
                        verifier_addr: Addr::unchecked(config.verifier),
                        component,
                        poll_interval,
                    },
                    linker,
                    store,
                )?;
                self.active_apps
                    .insert(name.to_string(), AppTrigger::Queue(handle));
            }
            _ => return Err(anyhow::anyhow!("unimplemented application trigger")),
        }

        Ok(())
    }
    pub async fn deactivate_app(&mut self, name: &str) -> Result<()> {
        let app = self
            .active_apps
            .get(name)
            .ok_or(anyhow::anyhow!("app not active"))?;
        match app {
            AppTrigger::Cron(id) => {
                // cancel CRON
                self.scheduler.remove(id).await?;
            }
            AppTrigger::Queue(handle) => {
                handle.abort();
            }
            AppTrigger::_Event => todo!(),
        }
        // remove app cache directory
        let app_cache_path = self.storage.path_for_app_cache(name);
        if !app_cache_path.is_dir() {
            tokio::fs::remove_dir_all(&app_cache_path).await?;
        }

        // remove from list of active apps
        self.active_apps.swap_remove(name);

        Ok(())
    }
}

pub type FileSystemOperator = Operator<FileSystemStorage>;

impl FileSystemOperator {
    /// Attempts to create an operator with the base file path for file system storage.
    pub async fn try_new(base_dir: PathBuf, envs: Vec<(String, String)>) -> Result<Self> {
        let storage = match FileSystemStorage::try_lock(&base_dir).await? {
            Some(storage) => storage,
            None => {
                return Err(anyhow::anyhow!(
                    "unable to acquire file system lock for path: {base_dir}",
                    base_dir = base_dir.display()
                ))
            }
        };
        Operator::new(storage, envs).await
    }
}

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
