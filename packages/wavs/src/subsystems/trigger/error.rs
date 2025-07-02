use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use utils::error::EvmClientError;
use wavs_types::{ByteArray, ChainName, ServiceID, WorkflowID};

use crate::{
    dispatcher::DispatcherCommand,
    subsystems::trigger::streams::local_command_stream::LocalStreamCommand,
};

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("EvmClient (chain {0}): {1}")]
    EvmClient(ChainName, EvmClientError),
    #[error("Evm subscription: {0}")]
    EvmSubscription(anyhow::Error),
    #[error("parse avs payload: {0}")]
    ParseAvsPayload(anyhow::Error),
    #[error("Cannot find service: {0}")]
    NoSuchService(ServiceID),
    #[error("Cannot find chain: {0}")]
    NoSuchChain(ChainName),
    #[error("Cannot find workflow: {0} / {1}")]
    NoSuchWorkflow(ServiceID, WorkflowID),
    #[error("Cannot find trigger data: {0}")]
    NoSuchTriggerData(usize),
    #[error("Unable to parse trigger data: {0}")]
    TriggerDataParse(String),
    #[error("Cannot find cosmos trigger contract: {0} / {1} / {2}")]
    NoSuchCosmosContractEvent(ChainName, layer_climb::prelude::Address, String),
    #[error("Cannot find EVM trigger contract: {0} / {1} / {2}")]
    NoSuchEvmContractEvent(ChainName, alloy_primitives::Address, ByteArray<32>),
    #[error("Cannot find block interval trigger: {0} / {1}")]
    NoSuchBlockIntervalTrigger(ChainName, u32),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ServiceID),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ServiceID, WorkflowID),
    #[error("Cron scheduling error: {expression} / {reason}")]
    Cron { expression: String, reason: String },
    #[error("Interval start time cannot be after end time")]
    IntervalStartAfterEnd,
    #[error("Dispatcher command send error: {0}")]
    DispatcherCommandSendError(#[from] SendError<DispatcherCommand>),
    #[error("Local command send error: {0}")]
    LocalCommandError(#[from] SendError<LocalStreamCommand>),
}
