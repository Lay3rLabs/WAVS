use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{runtime::Runtime, sync::mpsc};

use super::{Trigger, ID};

pub trait TriggerManager {
    /// Start running the trigger manager.
    /// This can create it's own default runtime or use the runtime passed in.
    /// This should only be called once in the lifetime of the object.
    fn start(&self, rt: Option<Arc<Runtime>>) -> mpsc::Receiver<TriggerAction>;

    fn add_trigger(&self, trigger: TriggerData) -> Result<(), TriggerError>;

    /// Remove one particular trigger
    fn remove_trigger(&self, service_id: ID, workflow_id: ID) -> Result<(), TriggerError>;

    /// Remove all workflows for one service
    fn remove_service(&self, service_id: ID) -> Result<(), TriggerError>;

    /// List all registered triggers, by service ID
    fn list_triggers(&self, service_id: ID) -> Result<Vec<TriggerData>, TriggerError>;
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
