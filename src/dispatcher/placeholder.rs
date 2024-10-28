use std::sync::Arc;

use tokio::runtime::{Handle, Runtime};

use crate::config::Config;

#[derive(Clone)]
pub struct Dispatcher {
    runtime: Option<Arc<Runtime>>,
    pub config: Arc<Config>,
}

impl Dispatcher {
    pub fn new(config: Config) -> Self {
        // Start a new tokio runtime to run our server
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4) // Configure as needed
            .enable_all()
            .build()
            .unwrap();

        Self {
            config: Arc::new(config),
            runtime: Some(Arc::new(runtime)),
        }
    }

    pub fn new_without_runtime(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            runtime: None,
        }
    }

    pub fn async_handle(&self) -> Handle {
        match self.runtime.as_ref() {
            Some(runtime) => runtime.handle().clone(),
            None => Handle::current(),
        }
    }
}
