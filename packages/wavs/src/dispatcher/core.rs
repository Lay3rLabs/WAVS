use std::sync::Arc;

use crate::engine::runner::MultiEngineRunner;
use crate::engine::WasmEngine;
use crate::submission::core::CoreSubmission;
use crate::triggers::core::CoreTriggerManager;
use crate::{config::Config, service::core::CoreServiceCache};
use utils::storage::fs::FileStorage;

use super::generic::{Dispatcher, DispatcherError};

pub type CoreDispatcher = Dispatcher<
    CoreTriggerManager,
    MultiEngineRunner<Arc<WasmEngine<FileStorage>>>,
    CoreSubmission,
    CoreServiceCache,
>;

impl CoreDispatcher {
    pub fn new_core(config: &Config) -> Result<CoreDispatcher, DispatcherError> {
        let file_storage = FileStorage::new(config.data.join("ca"))?;

        let triggers = CoreTriggerManager::new(config)?;

        let app_storage = config.data.join("app");
        let engine = Arc::new(WasmEngine::new(
            file_storage,
            app_storage,
            config.wasm_lru_size,
            config.chains.clone(),
            config.registry_domain.clone(),
        ));
        let engine = MultiEngineRunner::new(engine, config.wasm_threads);

        let submission = CoreSubmission::new(config)?;

        let service_manager = CoreServiceCache::new(config);

        Self::new(
            triggers,
            engine,
            submission,
            service_manager,
            config.data.join("db"),
        )
    }
}
