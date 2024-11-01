use crate::{
    apis::engine::{Engine, EngineError},
    engine::{identity::IdentityEngine, mock::MockEngine},
    submission::mock::MockSubmission,
    triggers::mock::MockTriggerManager,
    Digest,
};

use super::generic::Dispatcher;

pub type MockDispatcher = Dispatcher<MockTriggerManager, MockDispatcherAnyEngine, MockSubmission>;

pub struct MockDispatcherBuilder {
    pub triggers: MockTriggerManager,
    pub engine: MockDispatcherAnyEngine,
    pub submission: MockSubmission,
}

impl Default for MockDispatcherBuilder {
    fn default() -> Self {
        Self {
            triggers: MockTriggerManager::new(),
            engine: MockDispatcherAnyEngine::Identity(IdentityEngine),
            submission: MockSubmission::new(),
        }
    }
}

impl MockDispatcherBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_identity_engine(mut self) -> Self {
        self.engine = MockDispatcherAnyEngine::Identity(IdentityEngine);
        self
    }

    pub fn with_mock_engine(mut self, engine: MockEngine) -> Self {
        self.engine = MockDispatcherAnyEngine::Mock(engine);
        self
    }

    pub fn build(self) -> MockDispatcher {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        MockDispatcher::new(self.triggers, self.engine, self.submission, temp_file).unwrap()
    }
}

pub enum MockDispatcherAnyEngine {
    Identity(IdentityEngine),
    Mock(MockEngine),
}

impl Engine for MockDispatcherAnyEngine {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        match self {
            MockDispatcherAnyEngine::Identity(engine) => engine.store_wasm(bytecode),
            MockDispatcherAnyEngine::Mock(engine) => engine.store_wasm(bytecode),
        }
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        match self {
            MockDispatcherAnyEngine::Identity(engine) => engine.list_digests(),
            MockDispatcherAnyEngine::Mock(engine) => engine.list_digests(),
        }
    }

    fn execute_queue(
        &self,
        digest: Digest,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        match self {
            MockDispatcherAnyEngine::Identity(engine) => {
                engine.execute_queue(digest, request, timestamp)
            }
            MockDispatcherAnyEngine::Mock(engine) => {
                engine.execute_queue(digest, request, timestamp)
            }
        }
    }
}
