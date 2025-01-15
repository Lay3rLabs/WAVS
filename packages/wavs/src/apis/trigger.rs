use alloy::primitives::LogData;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use utils::config::ChainConfigs;

use crate::AppContext;

use utils::{IDError, ServiceID, WorkflowID};

// The TriggerManager reacts to these triggers
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    // A contract that emits an event
    CosmosContractEvent {
        address: Address,
        chain_id: String,
        event_type: String,
    },
    EthContractEvent {
        address: Address,
        chain_id: String,
        event_hash: Vec<u8>,
    },
    // not a real trigger, just for testing
    Test,
}

impl Trigger {
    pub fn cosmos_contract_event(
        address: Address,
        chain_id: impl ToString,
        event_type: impl ToString,
    ) -> Self {
        Trigger::CosmosContractEvent {
            address,
            chain_id: chain_id.to_string(),
            event_type: event_type.to_string(),
        }
    }
    pub fn eth_contract_event(
        address: Address,
        chain_id: impl ToString,
        event_hash: impl AsRef<[u8]>,
    ) -> Self {
        Trigger::EthContractEvent {
            address,
            chain_id: chain_id.to_string(),
            event_hash: event_hash.as_ref().to_vec(),
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
        chain_name: impl ToString,
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
        chain_name: impl ToString,
        event_hash: impl AsRef<[u8]>,
    ) -> Result<Self, IDError> {
        Ok(Self {
            service_id: service_id.try_into()?,
            workflow_id: workflow_id.try_into()?,
            trigger: Trigger::eth_contract_event(contract_address, chain_name, event_hash),
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
        chain_name: String,
        /// The data that was emitted by the contract
        event: cosmwasm_std::Event,
        /// The block height where the event was emitted
        block_height: u64,
    },
    EthContractEvent {
        /// The address of the contract that emitted the event
        contract_address: Address,
        /// The chain name of the chain where the event was emitted
        chain_name: String,
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

    pub fn try_into_component_input_eth_contract_event(
        self,
        chain_configs: ChainConfigs,
    ) -> Result<crate::bindings::worlds::eth_contract_event::Input, TriggerError> {
        match self {
            TriggerData::EthContractEvent {
                contract_address,
                chain_name,
                log,
                block_height,
            } => {
                let contract = layer_wasi::bindings::interface::EthAddr::try_from(contract_address)
                    .map_err(TriggerError::Climb)?;
                let chain_name: String = chain_name.to_string();
                let event_log_data: layer_wasi::bindings::interface::EthEventLogData = log.into();
                let block_height: u64 = block_height;

                Ok(crate::bindings::worlds::eth_contract_event::Input {
                    contract: contract.into(),
                    chain_name,
                    event_log_data: event_log_data.into(),
                    block_height,
                    chain_configs: chain_configs.into(),
                })
            }
            _ => Err(TriggerError::TriggerToComponentWorldInputMismatch),
        }
    }

    pub fn try_into_component_input_cosmos_contract_event(
        self,
        chain_configs: ChainConfigs,
    ) -> Result<crate::bindings::worlds::cosmos_contract_event::Input, TriggerError> {
        match self {
            TriggerData::CosmosContractEvent {
                contract_address,
                chain_name,
                event,
                block_height,
            } => {
                let contract =
                    layer_wasi::bindings::interface::CosmosAddr::try_from(contract_address)
                        .map_err(TriggerError::Climb)?;
                let chain_name: String = chain_name.to_string();
                let event: layer_wasi::bindings::interface::CosmosEvent = event.into();
                let block_height: u64 = block_height;

                Ok(crate::bindings::worlds::cosmos_contract_event::Input {
                    contract: contract.into(),
                    chain_name,
                    event: event.into(),
                    block_height,
                    chain_configs: chain_configs.into(),
                })
            }
            _ => Err(TriggerError::TriggerToComponentWorldInputMismatch),
        }
    }

    pub fn try_into_component_input_any_contract_event(
        self,
        chain_configs: ChainConfigs,
    ) -> Result<crate::bindings::worlds::any_contract_event::Input, TriggerError> {
        match self {
            TriggerData::CosmosContractEvent {
                contract_address,
                chain_name,
                event,
                block_height,
            } => {
                let contract = layer_wasi::bindings::interface::AnyAddr::from(contract_address);
                let chain_name: String = chain_name.to_string();
                let event: layer_wasi::bindings::interface::AnyEvent = event.into();
                let block_height: u64 = block_height;

                Ok(crate::bindings::worlds::any_contract_event::Input {
                    contract: contract.into(),
                    chain_name,
                    event: event.into(),
                    block_height,
                    chain_configs: chain_configs.into(),
                })
            }
            TriggerData::EthContractEvent {
                contract_address,
                chain_name,
                log,
                block_height,
            } => {
                let contract = layer_wasi::bindings::interface::AnyAddr::from(contract_address);
                let chain_name: String = chain_name.to_string();
                let event: layer_wasi::bindings::interface::AnyEvent = log.into();
                let block_height: u64 = block_height;

                Ok(crate::bindings::worlds::any_contract_event::Input {
                    contract: contract.into(),
                    chain_name,
                    event: event.into(),
                    block_height,
                    chain_configs: chain_configs.into(),
                })
            }
            _ => Err(TriggerError::TriggerToComponentWorldInputMismatch),
        }
    }

    pub fn try_into_component_input_raw(self) -> Result<Vec<u8>, TriggerError> {
        match self {
            TriggerData::Raw(data) => Ok(data),
            _ => Err(TriggerError::TriggerToComponentWorldInputMismatch),
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
    #[error("Trigger to component world input mismatch")]
    TriggerToComponentWorldInputMismatch,
    #[error("Cannot find cosmos trigger contract: {0} / {1} / {2}")]
    NoSuchCosmosContractEvent(String, Address, String),
    #[error("Cannot find eth trigger contract: {0} / {1} / {2}")]
    NoSuchEthContractEvent(String, Address, String),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ServiceID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ServiceID, WorkflowID),
    #[error("Cosmos Contract Event already registered: {0} / {1} / {2}")]
    CosmosContractEventAlreadyRegistered(String, Address, String),
    #[error("Eth Contract Event already registered: {0} / {1} / {2}")]
    EthContractEventAlreadyRegistered(String, Address, String),
}
