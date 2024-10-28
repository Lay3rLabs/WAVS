use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use super::{Trigger, ID};

pub struct TriggerManager {
    // TODO: implement this
}

impl TriggerManager {
    /// Create a new trigger manager.
    /// This returns the manager and a receiver for the trigger actions.
    /// Internally, all triggers may run in an async runtime and send results to the receiver.
    /// Externally, the Dispatcher can read the incoming tasks either sync or async
    pub fn create() -> (Self, mpsc::Receiver<TriggerAction>) {
        todo!();
    }

    pub fn add_trigger(&self, _trigger: TriggerData) -> Result<(), TriggerError> {
        todo!();
    }

    /// Remove one particular workflow
    pub fn remove_workflow(&self, _service_id: ID, _workflow_id: ID) -> Result<(), TriggerError> {
        todo!();
    }

    /// Remove all workflows for one service
    pub fn remove_service(&self, _service_id: ID) -> Result<(), TriggerError> {
        todo!();
    }

    /// List all registered triggers, by service ID
    pub fn list_triggers(&self, _service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        todo!();
    }
}

/// Internal description of a registered trigger, to be indexed by associated IDs
pub struct TriggerData {
    pub service_id: ID,
    pub workflow_id: ID,
    pub trigger: Trigger,
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TriggerAction {
    /// Identify which service and workflow this came from
    pub service_id: ID,
    pub workflow_id: ID,

    /// The data we got from the trigger
    pub result: TriggerResult,
}

/// This is the actual data we got from the trigger, used to feed into the component
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TriggerResult {
    Queue {
        /// The id from the task queue
        task_id: String,
        /// The input data associated with that task
        payload: Vec<u8>, // TODO: type with better serialization - Binary or serde_json::Value
    },
}

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("Cannot find service: {0}")]
    NoSuchService(ID),
    #[error("Cannot find workflow: {0} / {1}")]
    NoSuchWorkflow(ID, ID),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ID, ID),
}
