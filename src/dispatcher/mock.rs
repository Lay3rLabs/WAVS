use crate::{
    engine::identity::IdentityEngine, submission::mock::MockSubmission,
    triggers::mock::MockTriggerManager,
};

use super::generic::Dispatcher;

pub type MockDispatcher = Dispatcher<MockTriggerManager, IdentityEngine, MockSubmission>;

pub struct MockDispatcherBuilder {
    pub triggers: MockTriggerManager,
    pub engine: IdentityEngine,
    pub submission: MockSubmission,
}

impl MockDispatcherBuilder {
    pub fn new() -> Self {
        Self {
            triggers: MockTriggerManager::new(),
            engine: IdentityEngine,
            submission: MockSubmission::new(),
        }
    }

    pub fn build(self) -> MockDispatcher {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        MockDispatcher::new(self.triggers, self.engine, self.submission, temp_file).unwrap()
    }
}