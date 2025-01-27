use alloy::primitives::LogData;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::AppContext;

use utils::{types::ChainName, IDError, ServiceID, WorkflowID};

// The TriggerManager reacts to these triggers
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    // A contract that emits an event
    CosmosContractEvent {
        address: Address,
        chain_name: ChainName,
        event_type: String,
    },
    EthContractEvent {
        address: Address,
        chain_name: ChainName,
        event_hash: [u8; 32],
    },
    // not a real trigger, just for testing
    Manual,
}

impl Trigger {
    pub fn cosmos_contract_event(
        address: Address,
        chain_name: impl Into<ChainName>,
        event_type: impl ToString,
    ) -> Self {
        Trigger::CosmosContractEvent {
            address,
            chain_name: chain_name.into(),
            event_type: event_type.to_string(),
        }
    }
    pub fn eth_contract_event(
        address: Address,
        chain_name: impl Into<ChainName>,
        event_hash: [u8; 32],
    ) -> Self {
        Trigger::EthContractEvent {
            address,
            chain_name: chain_name.into(),
            event_hash,
        }
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
    pub fn cosmos_contract_event(
        service_id: impl TryInto<ServiceID, Error = IDError>,
        workflow_id: impl TryInto<WorkflowID, Error = IDError>,
        contract_address: Address,
        chain_name: impl Into<ChainName>,
        event_type: impl ToString,
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::cosmos_contract_event(contract_address, chain_name, event_type),
        })
    }

    pub fn eth_contract_event(
        service_id: impl TryInto<ServiceID, Error = IDError>,
        workflow_id: impl TryInto<WorkflowID, Error = IDError>,
        contract_address: Address,
        chain_name: impl Into<ChainName>,
        event_hash: [u8; 32],
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::eth_contract_event(contract_address, chain_name, event_hash),
        })
    }

    pub fn manual(
        service_id: impl TryInto<ServiceID, Error = IDError>,
        workflow_id: impl TryInto<WorkflowID, Error = IDError>,
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::Manual,
        })
    }
}

/// A bundle of the trigger and the associated data needed to take action on it
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TriggerAction {
    /// Identify which trigger this came from
    pub config: TriggerConfig,

    /// The data that came from the trigger
    pub data: TriggerData,
}

/// The data that came from the trigger
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TriggerData {
    CosmosContractEvent {
        /// The address of the contract that emitted the event
        contract_address: Address,
        /// The chain name of the chain where the event was emitted
        chain_name: ChainName,
        /// The data that was emitted by the contract
        event: cosmwasm_std::Event,
        /// The block height where the event was emitted
        block_height: u64,
    },
    EthContractEvent {
        /// The address of the contract that emitted the event
        contract_address: Address,
        /// The chain name of the chain where the event was emitted
        chain_name: ChainName,
        /// The raw event log
        log: LogData,
        /// The block height where the event was emitted
        block_height: u64,
    },
    Raw(Vec<u8>),
}

impl TriggerData {
    pub fn new_raw(data: impl AsRef<[u8]>) -> Self {
        TriggerData::Raw(data.as_ref().to_vec())
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
    #[error("Unable to parse trigger data: {0}")]
    TriggerDataParse(String),
    #[error("Cannot find cosmos trigger contract: {0} / {1} / {2}")]
    NoSuchCosmosContractEvent(ChainName, Address, String),
    #[error("Cannot find eth trigger contract: {0} / {1} / {2}")]
    NoSuchEthContractEvent(ChainName, Address, String),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ServiceID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ServiceID, WorkflowID),
    #[error("Cosmos Contract Event already registered: {0} / {1} / {2}")]
    CosmosContractEventAlreadyRegistered(ChainName, Address, String),
    #[error("Eth Contract Event already registered: {0} / {1} / {2}")]
    EthContractEventAlreadyRegistered(ChainName, Address, String),
}
