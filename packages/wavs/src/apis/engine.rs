use layer_climb::prelude::Address;
use thiserror::Error;

use utils::storage::CAStorageError;

use super::trigger::TriggerAction;
use wavs_types::{Component, ComponentID, Digest, ServiceConfig, ServiceID, WorkflowID};

pub trait Engine: Send + Sync {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError>;

    /// This will execute a component that implements one of our supported interfaces
    fn execute(
        &self,
        component: &Component,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError>;
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
        component: &Component,
        trigger: TriggerAction,
        service_config: &ServiceConfig,
    ) -> Result<Vec<u8>, EngineError> {
        self.as_ref().execute(component, trigger, service_config)
    }
}

// Note: I tried to pull this into an associated type of the trait,
// But then embedding this in DispatcherError got all kinds of ugly.
// We need to use anyhow if we want to allow this to be a trait associated type
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Storage: {0}")]
    Storage(#[from] CAStorageError),

    #[error{"IO: {0}"}]
    IO(#[from] std::io::Error),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ServiceID, WorkflowID),

    #[error("Unknown Component {0}")]
    UnknownComponent(ComponentID),

    #[error("Invalid Wasm bytecode")]
    InvalidWasmCode,

    #[error("Wasm doesn't match expected wit interface")]
    WasmInterfaceMismatch,

    #[error("No wasm found for digest {0}")]
    UnknownDigest(Digest),

    #[error("Component returned an error: {0}")]
    ComponentError(String),

    #[error{"invalid address: {0}"}]
    InvalidAddress(Address),

    #[error{"unable to get trigger data as component input: {0}"}]
    TriggerData(anyhow::Error),

    #[error{"{0}"}]
    Other(#[from] anyhow::Error),

    #[error("Max fuel consumed by WasmEngine for service: {0}, workflow: {1}")]
    OutOfFuel(ServiceID, WorkflowID),
}
