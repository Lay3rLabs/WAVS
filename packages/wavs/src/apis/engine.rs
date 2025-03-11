use serde::{Deserialize, Serialize};
use thiserror::Error;

use utils::storage::CAStorageError;

use wavs_types::{
    ComponentID, Digest, Permissions, ServiceConfig, ServiceID, TriggerAction, WorkflowID,
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ExecutionComponent {
    pub wasm: Digest,
    // What permissions this component has.
    // These are currently not enforced, you can pass in Default::default() for now
    pub permissions: Permissions,
}

pub trait Engine: Send + Sync {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError>;

    /// This will execute a component that implements one of our supported interfaces
    fn execute(
        &self,
        component: &ExecutionComponent,
        fuel_limit: Option<u64>,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Option<Vec<u8>>, EngineError>;
}

impl<E: Engine> Engine for std::sync::Arc<E> {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        self.as_ref().store_wasm(bytecode)
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        self.as_ref().list_digests()
    }

    fn execute(
        &self,
        component: &ExecutionComponent,
        fuel_limit: Option<u64>,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        self.as_ref()
            .execute(component, fuel_limit, trigger, service_config)
    }
}

// Note: I tried to pull this into an associated type of the trait,
// But then embedding this in DispatcherError got all kinds of ugly.
// We need to use anyhow if we want to allow this to be a trait associated type
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error{"Compile: {0}"}]
    Compile(anyhow::Error),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ServiceID, WorkflowID),

    #[error("Unknown Component {0}")]
    UnknownComponent(ComponentID),

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),

    #[error{"{0}"}]
    Engine(#[from] wavs_engine::EngineError),

    #[error{"Unable to send result after executing Service {0} / Workflow {1}"}]
    WasiResultSend(ServiceID, WorkflowID),

    #[error("No registry configured")]
    NoRegistry,
}
