use thiserror::Error;
use utils::{
    error::{ChainConfigError, EvmClientError},
    storage::db::DBError,
};
use wavs_types::{ChainName, EnvelopeError, EventId, ServiceID, WorkflowID};

pub type AggregatorResult<T> = Result<T, AggregatorError>;

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("Missing workflow: {workflow_id} for service: {service_id}")]
    MissingWorkflow {
        workflow_id: WorkflowID,
        service_id: ServiceID,
    },

    #[error("Service already registered: {0}")]
    RepeatService(ServiceID),

    #[error("No such service registered: {0}")]
    MissingService(ServiceID),

    #[error("DB: {0}")]
    DBError(#[from] DBError),

    #[error("Packet Validation: {0}")]
    PacketValidation(#[from] PacketValidationError),

    #[error("Chain Config: {0}")]
    ChainConfig(#[from] ChainConfigError),

    #[error("Evm: {0}")]
    EvmClient(#[from] EvmClientError),

    #[error("Envelope: {0}")]
    Envelope(#[from] EnvelopeError),

    #[error("Evm client create: {0:?}")]
    CreateEvmClient(anyhow::Error),

    #[error("Service manager validate(): {0:?}")]
    ServiceManagerValidate(alloy_contract::Error),

    #[error("Chain not found: {0}")]
    ChainNotFound(ChainName),

    #[error("Missing EVM credential")]
    MissingEvmCredential,

    #[error("Unexpected responses length: should be {responses}, got {aggregators}")]
    UnexpectedResponsesLength {
        responses: usize,
        aggregators: usize,
    },

    #[error("Block number: {0}")]
    BlockNumber(anyhow::Error),
}

#[derive(Error, Debug)]
pub enum PacketValidationError {
    #[error("Unexpected envelope difference")]
    EnvelopeDiff,

    #[error("Signer already in queue: {0}")]
    RepeatSigner(alloy_primitives::Address),

    #[error("Packets for event {0} already burned")]
    EventBurned(EventId),
}
