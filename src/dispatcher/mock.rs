use std::sync::Arc;

use crate::{
    engine::WasmEngine, storage::memory::MemoryStorage, submission::mock::MockSubmission,
    triggers::mock::MockTriggerManager,
};

use super::generic::Dispatcher;

pub type MockDispatcher =
    Dispatcher<MockTriggerManager, Arc<WasmEngine<MemoryStorage>>, MockSubmission>;

impl MockDispatcher {
    pub fn new_mock() -> Self {
        let file_storage = MemoryStorage::new();

        let triggers = MockTriggerManager::new();

        let engine = Arc::new(WasmEngine::new(file_storage));

        let submission = MockSubmission::new();

        let temp_file = tempfile::NamedTempFile::new().unwrap();

        Self::new(triggers, engine, submission, temp_file.as_ref()).unwrap()
    }
}
