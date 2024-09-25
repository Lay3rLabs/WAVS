use anyhow::{Context, Result};
use axum::{
    extract::DefaultBodyLimit,
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
use tokio_cron_scheduler::{Job, JobScheduler};
use wasm_pkg_client::Registry;
use wasmtime::{
    component::{Component, Linker},
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
use crate::cron_bindings;
use crate::task_bindings;
mod upload;
//mod update_application;

pub struct Operator<S>
where
    S: Storage,
{
    engine: Engine,
    scheduler: JobScheduler,
    queue_executor: QueueExecutor,
    active_apps: IndexMap<String, uuid::Uuid>,
    envs: Vec<(String, String)>,
    registry: Option<Registry>,
    storage: S,
}

impl<S: Storage + 'static> Operator<S> {
    pub async fn new(
        storage: S,
        envs: Vec<(String, String)>,
        registry: Option<Registry>,
    ) -> Result<Self> {
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
            registry,
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
            .route("/upload", post(upload::upload))
            .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
            .with_state(Arc::new(Mutex::new(self)));

        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("failed to bind to address `{bind_addr}`"))?;

        println!("Listening on {}", bind_addr);

        axum::serve::serve(listener, router.into_make_service()).await?;
        Ok(())
    }

    pub fn registry(&self) -> Option<&Registry> {
        self.registry.as_ref()
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

        let component = self.storage.get_wasm(app.digest()?, &self.engine).await?;
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
        match app.trigger {
            app::Trigger::Cron { schedule } => {
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
                                let output = instantiate_and_invoke(
                                    &envs,
                                    &app_cache_path,
                                    &engine,
                                    &linker,
                                    &component,
                                    TriggerRequest::Cron,
                                )
                                .await
                                .expect("Failed to instantiate component");
                                let output_string =
                                    std::str::from_utf8(&output).expect("Output is invalid utf8");
                                println!("Cron Job output was: {output_string}");
                            }
                        })
                    })?)
                    .await?;

                // TODO handle possible race condition on adding job

                // save the job ID in the active CRON jobs
                self.active_apps.insert(name.to_string(), id);
            }
            app::Trigger::Queue {
                task_queue_addr,
                hd_index,
                poll_interval,
            } => {
                let lay3r = self
                    .queue_executor
                    .builder
                    .hd_index(hd_index)
                    .build()
                    .await?;
                let schedule = format!("1/{poll_interval} * * * * *");
                let name = name.to_string();
                let name_copy = name.clone();
                let id = self
                    .scheduler
                    .add(Job::new_async(schedule.as_str(), move |_, _| {
                        Box::pin({
                            let envs = envs.clone();
                            let app_cache_path = app_cache_path.clone();
                            let engine = engine.clone();
                            let component = component.clone();
                            let linker = linker.clone();
                            let task_queue_addr = Addr::unchecked(&task_queue_addr);
                            let lay3r = lay3r.clone();
                            let name = name.clone();
                            async move {
                                let query = lavs_apis::tasks::CustomQueryMsg::Config {};
                                let config: lavs_apis::tasks::ConfigResponse =
                                    lay3r.query(&query, &task_queue_addr).await.unwrap();
                                let app = AppData {
                                    task_queue_addr: task_queue_addr.clone(),
                                    lay3r,
                                    verifier_addr: Addr::unchecked(config.verifier),
                                };
                                println!("Polling for tasks for application: {}...", &name);
                                let tasks = app.get_tasks().await.unwrap();
                                for t in tasks {
                                    println!("Task: {:?}", t);
                                    let request = serde_json::to_vec(&t.payload).unwrap();

                                    let output = instantiate_and_invoke(
                                        &envs,
                                        &app_cache_path,
                                        &engine,
                                        &linker,
                                        &component,
                                        TriggerRequest::Queue(request),
                                    )
                                    .await
                                    .expect("Failed to instantiate component");
                                    let output_string = std::str::from_utf8(&output)
                                        .expect("Output is invalid utf8");
                                    println!("Task output was: {output_string}");

                                    app.submit_result(t.id, output_string.to_string())
                                        .await
                                        .unwrap();
                                }
                            }
                        })
                    })?)
                    .await?;
                self.active_apps.insert(name_copy.to_string(), id);
            }
            _ => return Err(anyhow::anyhow!("unimplemented application trigger")),
        }

        Ok(())
    }
    pub async fn deactivate_app(&mut self, name: &str) -> Result<()> {
        let id = self
            .active_apps
            .get(name)
            .ok_or(anyhow::anyhow!("app not active"))?;
        self.scheduler.remove(id).await?;
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

enum TriggerRequest {
    Cron,
    Queue(Vec<u8>),
    _Event,
}

async fn instantiate_and_invoke(
    envs: &[(String, String)],
    app_cache_path: &PathBuf,
    engine: &Engine,
    linker: &Linker<Host>,
    component: &Component,
    trigger: TriggerRequest,
) -> Result<Vec<u8>, String> {
    let mut builder = WasiCtxBuilder::new();
    if !envs.is_empty() {
        builder.envs(envs);
    }
    builder
        .preopened_dir(app_cache_path, ".", DirPerms::all(), FilePerms::all())
        .expect("preopen failed");
    let ctx = builder.build();

    let host = Host {
        table: wasmtime::component::ResourceTable::new(),
        ctx,
        http: WasiHttpCtx::new(),
    };
    let mut store = wasmtime::Store::new(engine, host);
    match trigger {
        TriggerRequest::Cron => {
            let bindings = cron_bindings::CronJob::instantiate_async(&mut store, component, linker)
                .await
                .expect("Wasm instantiate failed");

            bindings
                .call_run_cron(&mut store)
                .await
                .expect("Failed to call invoke cron job")
        }
        TriggerRequest::Queue(bytes) => {
            let bindings =
                task_bindings::TaskQueue::instantiate_async(&mut store, component, linker)
                    .await
                    .expect("Wasm instantiate failed");

            let input = task_bindings::Input {
                timestamp: get_time(),
                bytes,
            };
            bindings
                .call_run_task(&mut store, &input)
                .await
                .expect("Failed to run task")
        }
        TriggerRequest::_Event => todo!(),
    }
}

fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub type FileSystemOperator = Operator<FileSystemStorage>;

impl FileSystemOperator {
    /// Attempts to create an operator with the base file path for file system storage.
    pub async fn try_new(
        base_dir: PathBuf,
        envs: Vec<(String, String)>,
        registry: Option<Registry>,
    ) -> Result<Self> {
        let storage = match FileSystemStorage::try_lock(&base_dir).await? {
            Some(storage) => storage,
            None => {
                return Err(anyhow::anyhow!(
                    "unable to acquire file system lock for path: {base_dir}",
                    base_dir = base_dir.display()
                ))
            }
        };
        Operator::new(storage, envs, registry).await
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
