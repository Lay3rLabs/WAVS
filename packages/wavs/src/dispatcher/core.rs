use std::sync::Arc;

use crate::engine::runner::MultiEngineRunner;
use crate::engine::WasmEngine;
use crate::submission::core::CoreSubmission;
use crate::{config::Config, trigger_manager::TriggerManager};
use utils::{storage::fs::FileStorage, telemetry::WavsMetrics};

use super::generic::{Dispatcher, DispatcherError};

pub type CoreDispatcher =
    Dispatcher<MultiEngineRunner<Arc<WasmEngine<FileStorage>>>, CoreSubmission>;

impl CoreDispatcher {
    pub fn new_core(
        config: &Config,
        metrics: WavsMetrics,
    ) -> Result<CoreDispatcher, DispatcherError> {
        let file_storage = FileStorage::new(config.data.join("ca"))?;

        let triggers = TriggerManager::new(config, metrics.trigger)?;

        let app_storage = config.data.join("app");
        let engine = Arc::new(WasmEngine::new(
            file_storage,
            app_storage,
            config.wasm_lru_size,
            config.chains.clone(),
            Some(config.max_wasm_fuel),
            Some(config.max_execution_seconds),
            metrics.engine,
        ));
        let engine = MultiEngineRunner::new(engine, config.wasm_threads);

        let submission = CoreSubmission::new(config, metrics.submission)?;

        Self::new(
            triggers,
            engine,
            submission,
            config.chains.clone(),
            config.data.join("db"),
            metrics.dispatcher,
            config.ipfs_gateway.clone(),
        )
    }
}
