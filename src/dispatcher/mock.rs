use std::sync::Arc;

use crate::{
    apis::dispatcher::DispatchManager, context::AppContext, engine::WasmEngine,
    storage::memory::MemoryStorage, submission::mock::MockSubmission,
    triggers::mock::MockTriggerManager,
};

use super::{generic::Dispatcher, DispatcherError};

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

impl DispatchManager for MockDispatcher {
    type Error = DispatcherError;

    fn start(&self, _ctx: AppContext) -> Result<(), Self::Error> {
        todo!()
    }

    fn run_trigger(
        &self,
        _action: crate::apis::trigger::TriggerAction,
    ) -> Result<Option<crate::apis::submission::ChainMessage>, Self::Error> {
        todo!()
    }

    fn store_component(
        &self,
        _source: crate::apis::dispatcher::WasmSource,
    ) -> Result<crate::Digest, Self::Error> {
        todo!()
    }

    fn add_service(&self, _service: crate::apis::dispatcher::Service) -> Result<(), Self::Error> {
        todo!()
    }

    fn remove_service(&self, _id: crate::apis::ID) -> Result<(), Self::Error> {
        todo!()
    }

    fn list_services(&self) -> Result<Vec<crate::apis::dispatcher::Service>, Self::Error> {
        todo!()
    }
}
