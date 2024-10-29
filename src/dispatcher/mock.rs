use crate::{
    context::AppContext, engine::WasmEngine, storage::fs::FileStorage,
    submission::mock::MockSubmission, triggers::mock::MockTriggerManager,
};

use super::generic::Dispatcher;

pub type MockDispatcher = Dispatcher<MockTriggerManager, WasmEngine<FileStorage>, MockSubmission>;

impl MockDispatcher {
    pub fn new_mock(ctx: AppContext) -> Self {
        let file_storage = FileStorage::new(ctx.config.data.join("ca")).unwrap();

        let triggers = MockTriggerManager::new();

        let engine = WasmEngine::new(file_storage);

        let submission = MockSubmission::new();

        Self::new(triggers, engine, submission, ctx.config.data.join("db")).unwrap()
    }
}
