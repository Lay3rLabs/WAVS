use lavs_apis::id::TaskId;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::context::AppContext;

use super::{IDError, ServiceID, WorkflowID};

// The TriggerManager reacts to these triggers
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    // TODO: add this variant later, not for now
    // #[serde(rename_all = "camelCase")]
    // Cron { schedule: String },
    #[serde(rename_all = "camelCase")]
    LayerQueue {
        // FIXME: add some chain name. right now all triggers are on one chain
        task_queue_addr: Address,
        /// Frequency in seconds to poll the task queue (doubt this is over 3600 ever, but who knows)
        poll_interval: u32,
    },
    EthQueue {
        // FIXME: add some chain name. right now all triggers are on one chain
        // For right now this is NOT actually a generic task queue, it's AVS-specific
        task_queue_addr: Address,
    },
}

impl Trigger {
    pub fn layer_queue(task_queue_addr: Address, poll_interval: u32) -> Self {
        Trigger::LayerQueue {
            task_queue_addr,
            poll_interval,
        }
    }

    pub fn eth_queue(task_queue_addr: Address) -> Self {
        Trigger::EthQueue { task_queue_addr }
    }
}
pub trait TriggerManager: Send + Sync {
    /// Start running the trigger manager.
    /// This should only be called once in the lifetime of the object
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError>;

    fn add_trigger(&self, trigger: TriggerConfig) -> Result<(), TriggerError>;

    /// Remove one particular trigger
    fn remove_trigger(
        &self,
        service_id: ServiceID,
        workflow_id: WorkflowID,
    ) -> Result<(), TriggerError>;

    /// Remove all workflows for one service
    fn remove_service(&self, service_id: ServiceID) -> Result<(), TriggerError>;

    /// List all registered triggers, by service ID
    fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError>;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
// Trigger with metadata so it can be identified in relation to services and workflows
pub struct TriggerConfig {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub trigger: Trigger,
}

impl TriggerConfig {
    pub fn layer_queue(
        service_id: impl TryInto<ServiceID, Error = IDError>,
        workflow_id: impl TryInto<WorkflowID, Error = IDError>,
        task_queue_addr: Address,
        poll_interval: u32,
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::layer_queue(task_queue_addr, poll_interval),
        })
    }

    pub fn eth_queue(
        service_id: impl TryInto<ServiceID, Error = IDError>,
        workflow_id: impl TryInto<WorkflowID, Error = IDError>,
        task_queue_addr: Address,
        task_queue_erc1271: Address,
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::eth_queue(task_queue_addr, task_queue_erc1271),
        })
    }
}

/// A bundle of the trigger and the associated data needed to take action on it
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TriggerAction {
    /// Identify which trigger this came from
    pub config: TriggerConfig,

    /// The data that's required for the trigger to be processed
    pub data: TriggerData,
}

/// This is the actual data we got from the trigger, used to feed into the component
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TriggerData {
    Queue {
        /// The id from the task queue
        task_id: TaskId,
        /// The input data associated with that task
        payload: Vec<u8>, // TODO: type with better serialization - Binary or serde_json::Value
    },
}

impl TriggerData {
    pub fn queue(task_id: TaskId, payload: &[u8]) -> Self {
        TriggerData::Queue {
            task_id,
            payload: payload.to_vec(),
        }
    }
}

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("ethereum: {0}")]
    Ethereum(anyhow::Error),
    #[error("parse avs payload: {0}")]
    ParseAvsPayload(anyhow::Error),
    #[error("Cannot find service: {0}")]
    NoSuchService(ServiceID),
    #[error("Cannot find workflow: {0} / {1}")]
    NoSuchWorkflow(ServiceID, WorkflowID),
    #[error("Cannot find trigger data: {0}")]
    NoSuchTriggerData(usize),
    #[error("Cannot find trigger data: {0}")]
    NoSuchTaskQueueTrigger(Address),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ServiceID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ServiceID, WorkflowID),
}
