use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    apis::{
        engine::{Engine, EngineError},
        trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager},
        ID,
    },
    context::AppContext,
    engine::{identity::IdentityEngine, mock::MockEngine},
    submission::mock::MockSubmission,
    triggers::mock::{MockTriggerManagerChannel, MockTriggerManagerVec},
    Digest,
};

use super::generic::Dispatcher;

pub type MockDispatcher =
    Dispatcher<MockDispatcherAnyTriggerManager, MockDispatcherAnyEngine, MockSubmission>;

pub struct MockDispatcherBuilder {
    pub triggers: MockDispatcherAnyTriggerManager,
    pub engine: MockDispatcherAnyEngine,
    pub submission: MockSubmission,
}

impl Default for MockDispatcherBuilder {
    fn default() -> Self {
        Self {
            triggers: MockDispatcherAnyTriggerManager::Vec(MockTriggerManagerVec::new()),
            engine: MockDispatcherAnyEngine::Identity(IdentityEngine),
            submission: MockSubmission::new(),
        }
    }
}

impl MockDispatcherBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_trigger_manager_vec(mut self, trigger_manager: MockTriggerManagerVec) -> Self {
        self.triggers = MockDispatcherAnyTriggerManager::Vec(trigger_manager);
        self
    }

    pub fn with_trigger_manager_channel(
        mut self,
        trigger_manager: MockTriggerManagerChannel,
    ) -> Self {
        self.triggers = MockDispatcherAnyTriggerManager::Channel(trigger_manager);
        self
    }

    pub fn with_identity_engine(mut self) -> Self {
        self.engine = MockDispatcherAnyEngine::Identity(IdentityEngine);
        self
    }

    pub fn with_mock_engine(mut self, engine: MockEngine) -> Self {
        self.engine = MockDispatcherAnyEngine::Mock(engine);
        self
    }

    pub fn build(self) -> Arc<MockDispatcher> {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        Arc::new(
            MockDispatcher::new(self.triggers, self.engine, self.submission, temp_file).unwrap(),
        )
    }
}

pub enum MockDispatcherAnyTriggerManager {
    Vec(MockTriggerManagerVec),
    Channel(MockTriggerManagerChannel),
}

impl MockDispatcherAnyTriggerManager {
    pub fn as_vec(&self) -> &MockTriggerManagerVec {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => manager,
            _ => panic!("Expected Vec, got Channel"),
        }
    }

    pub fn as_channel(&self) -> &MockTriggerManagerChannel {
        match self {
            MockDispatcherAnyTriggerManager::Channel(manager) => manager,
            _ => panic!("Expected Channel, got Vec"),
        }
    }
}

impl TriggerManager for MockDispatcherAnyTriggerManager {
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => manager.start(ctx),
            MockDispatcherAnyTriggerManager::Channel(manager) => manager.start(ctx),
        }
    }

    fn add_trigger(&self, trigger: TriggerData) -> Result<(), TriggerError> {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => manager.add_trigger(trigger),
            MockDispatcherAnyTriggerManager::Channel(manager) => manager.add_trigger(trigger),
        }
    }

    fn remove_trigger(&self, service_id: ID, workflow_id: ID) -> Result<(), TriggerError> {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => {
                manager.remove_trigger(service_id, workflow_id)
            }
            MockDispatcherAnyTriggerManager::Channel(manager) => {
                manager.remove_trigger(service_id, workflow_id)
            }
        }
    }

    fn remove_service(&self, service_id: ID) -> Result<(), TriggerError> {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => manager.remove_service(service_id),
            MockDispatcherAnyTriggerManager::Channel(manager) => manager.remove_service(service_id),
        }
    }

    fn list_triggers(&self, service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        match self {
            MockDispatcherAnyTriggerManager::Vec(manager) => manager.list_triggers(service_id),
            MockDispatcherAnyTriggerManager::Channel(manager) => manager.list_triggers(service_id),
        }
    }
}

pub enum MockDispatcherAnyEngine {
    Identity(IdentityEngine),
    Mock(MockEngine),
}

impl MockDispatcherAnyEngine {
    pub fn as_mock(&self) -> &MockEngine {
        match self {
            MockDispatcherAnyEngine::Mock(engine) => engine,
            _ => panic!("Expected Mock, got Identity"),
        }
    }

    pub fn as_identity(&self) -> &IdentityEngine {
        match self {
            MockDispatcherAnyEngine::Identity(engine) => engine,
            _ => panic!("Expected Identity, got Mock"),
        }
    }
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
