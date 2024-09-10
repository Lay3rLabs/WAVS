use anyhow::{Context, Result};
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_cron_scheduler::JobScheduler;
use wasmtime::{Config, Engine};

use crate::storage::{FileSystemStorage, Storage};
mod add_application;
mod delete_application;
mod list_applications;
mod update_application;

pub struct Operator<S>
where
    S: Storage,
{
    engine: Engine,
    scheduler: JobScheduler,
    storage: S,
}

impl<S: Storage + 'static> Operator<S> {
    pub async fn new(storage: S) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();

        let scheduler = JobScheduler::new().await?;

        Ok(Operator {
            engine,
            scheduler,
            storage,
        })
    }

    pub async fn serve(self, bind_addr: SocketAddr) -> Result<()> {
        let router = Router::new()
            .route("/app", get(list_applications::list))
            .route("/app", post(add_application::add))
            .route("/app", put(update_application::update))
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
}

pub type FileSystemOperator = Operator<FileSystemStorage>;

impl FileSystemOperator {
    /// Attempts to create an operator with the base file path for file system storage.
    pub async fn try_new(base_dir: PathBuf) -> Result<Self> {
        let storage = match FileSystemStorage::try_lock(&base_dir)? {
            Some(storage) => storage,
            None => {
                return Err(anyhow::anyhow!(
                    "unable to acquire file system lock for path: {base_dir}",
                    base_dir = base_dir.display()
                ))
            }
        };
        Ok(Operator::new(storage).await?)
    }
}
