use lavs_apis::id::TaskId;
use layer_climb::prelude::Address;
use thiserror::Error;
use utils::layer_contract_client::TriggerId;

use crate::{storage::CAStorageError, Digest};

use super::dispatcher::{Component, ServiceConfig};
use utils::{ComponentID, ServiceID, WorkflowID};

pub trait Engine: Send + Sync {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError>;

    // TODO: paginate this
    fn list_digests(&self) -> Result<Vec<Digest>, EngineError>;

    /// This will execute a contract that implements the layer_avs:task-queue wit interface
    fn execute_queue(
        &self,
        component: &Component,
        service_config: &ServiceConfig,
        service_id: &ServiceID,
        task_id: TaskId,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError>;

    /// This will execute a contract that implements the layer_avs:eth-event wit interface
    fn execute_eth_event(
        &self,
        component: &Component,
        service_config: &ServiceConfig,
        service_id: &ServiceID,
        workflow_id: &WorkflowID,
        trigger_id: TriggerId,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, EngineError>;
}

impl<E: Engine> Engine for std::sync::Arc<E> {
    fn store_wasm(&self, bytecode: &[u8]) -> Result<Digest, EngineError> {
        self.as_ref().store_wasm(bytecode)
    }

    fn list_digests(&self) -> Result<Vec<Digest>, EngineError> {
        self.as_ref().list_digests()
    }

    fn execute_queue(
        &self,
        component: &Component,
        service_config: &ServiceConfig,
        service_id: &ServiceID,
        task_id: TaskId,
        request: Vec<u8>,
        timestamp: u64,
    ) -> Result<Vec<u8>, EngineError> {
        self.as_ref().execute_queue(
            component,
            service_config,
            service_id,
            task_id,
            request,
            timestamp,
        )
    }

    fn execute_eth_event(
        &self,
        component: &Component,
        service_config: &ServiceConfig,
        service_id: &ServiceID,
        workflow_id: &WorkflowID,
        trigger_id: TriggerId,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, EngineError> {
        self.as_ref().execute_eth_event(
            component,
            service_config,
            service_id,
            workflow_id,
            trigger_id,
            payload,
        )
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

    #[error{"{0}"}]
    Other(#[from] anyhow::Error),

    #[error("Max fuel consumed by WasmEngine for service: {0}. Id: {1}")]
    OutOfFuel(ServiceID, u64),
}
