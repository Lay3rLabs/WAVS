use iri_string::types::CreationError;
use thiserror::Error;
use utils::error::EvmClientError;
use wavs_types::{ByteArray, ChainKey, ServiceId, WorkflowId};

use crate::{
    dispatcher::DispatcherCommand, services::ServicesError, subsystems::trigger::TriggerCommand,
};

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("EvmClient (chain {0}): {1}")]
    EvmClient(ChainKey, EvmClientError),
    #[error("EvmClient (chain {0}): missing websocket endpoint")]
    EvmMissingWebsocket(ChainKey),
    #[error("Evm subscription: {0}")]
    EvmSubscription(anyhow::Error),
    #[error("parse avs payload: {0}")]
    ParseAvsPayload(anyhow::Error),
    #[error("Cannot find service: {0}")]
    NoSuchService(ServiceId),
    #[error("Cannot find chain: {0}")]
    NoSuchChain(ChainKey),
    #[error("Cannot find workflow: {0} / {1}")]
    NoSuchWorkflow(ServiceId, WorkflowId),
    #[error("Cannot find trigger data: {0}")]
    NoSuchTriggerData(usize),
    #[error("Unable to parse trigger data: {0}")]
    TriggerDataParse(String),
    #[error("Cannot find cosmos trigger contract: {0} / {1} / {2}")]
    NoSuchCosmosContractEvent(ChainKey, layer_climb::prelude::Address, String),
    #[error("Cannot find EVM trigger contract: {0} / {1} / {2}")]
    NoSuchEvmContractEvent(ChainKey, alloy_primitives::Address, ByteArray<32>),
    #[error("Cannot find block interval trigger: {0} / {1}")]
    NoSuchBlockIntervalTrigger(ChainKey, u32),
    #[error("Service exists, cannot register again: {0}")]
    ServiceAlreadyExists(ServiceId),
    #[error("Workflow exists, cannot register again: {0} / {1}")]
    WorkflowAlreadyExists(ServiceId, WorkflowId),
    #[error("Cron scheduling error: {expression} / {reason}")]
    Cron { expression: String, reason: String },
    #[error("Interval start time cannot be after end time")]
    IntervalStartAfterEnd,
    #[error("Config error: {0}")]
    Config(String),
    #[error("Dispatcher command send error: {0}")]
    DispatcherCommandSendError(#[from] Box<crossbeam::channel::SendError<DispatcherCommand>>),
    #[error("Local command send error: {0}")]
    LocalCommandError(#[from] tokio::sync::mpsc::error::SendError<TriggerCommand>),
    #[error("Unable to convert event index: {0}")]
    EventIndexConversion(std::num::TryFromIntError),
    #[error("could not encode EventId {0:?}")]
    EncodeEventId(anyhow::Error),
    #[error("could not get service {0:?}")]
    Services(ServicesError),
    #[error("URI creation error: {0}")]
    URICreation(#[from] CreationError<String>),
    #[error("Jetstream connection error: {0}")]
    JetstreamConnection(String),
    #[error("Jetstream configuration error: {0}")]
    JetstreamConfig(String),
    #[error("Jetstream parsing error: {0}")]
    JetstreamParse(String),
    #[error("Hypercore error: {0}")]
    Hypercore(String),
}
