use crate::{
    engine::identity::IdentityEngine, submission::mock::MockSubmission,
    triggers::mock::MockTriggerManager,
};

use super::generic::Dispatcher;

pub type MockDispatcher = Dispatcher<MockTriggerManager, IdentityEngine, MockSubmission>;

/// Note: this is more or less useless, as we will want to actually to configure these items more.
impl MockDispatcher {
    pub fn new_mock() -> Self {
        let triggers = MockTriggerManager::new();

        let engine = IdentityEngine;

        let submission = MockSubmission::new();

        let temp_file = tempfile::NamedTempFile::new().unwrap();

        Self::new(triggers, engine, submission, temp_file.as_ref()).unwrap()
    }
}
