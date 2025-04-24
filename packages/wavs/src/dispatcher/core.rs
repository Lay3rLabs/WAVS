use std::sync::Arc;

use crate::config::Config;
use crate::engine::runner::MultiEngineRunner;
use crate::engine::WasmEngine;
use crate::submission::core::CoreSubmission;
use crate::triggers::core::CoreTriggerManager;
use utils::{storage::fs::FileStorage, telemetry::WavsMetrics};

use super::generic::{Dispatcher, DispatcherError};

pub type CoreDispatcher =
    Dispatcher<CoreTriggerManager, MultiEngineRunner<Arc<WasmEngine<FileStorage>>>, CoreSubmission>;

impl CoreDispatcher {
    pub fn new_core(
        config: &Config,
        metrics: WavsMetrics,
    ) -> Result<CoreDispatcher, DispatcherError> {
        let file_storage = FileStorage::new(config.data.join("ca"))?;

        let triggers = CoreTriggerManager::new(config, metrics.trigger)?;

        let app_storage = config.data.join("app");
        let engine = Arc::new(WasmEngine::new(
            file_storage,
            app_storage,
            config.wasm_lru_size,
            config.chains.clone(),
            config.registry_domain.clone(),
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
        )
    }
}
