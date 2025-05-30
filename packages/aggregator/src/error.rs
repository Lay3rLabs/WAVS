use thiserror::Error;
use utils::{
    error::{ChainConfigError, EvmClientError},
    storage::db::DBError,
};
use wavs_types::{ChainName, EnvelopeError, ServiceID, ServiceManagerError, WorkflowID};

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
    ServiceManagerValidateKnown(ServiceManagerError),

    #[error("Service manager validate(): {0}")]
    ServiceManagerValidateAnyRevert(String),

    #[error("Service manager validate(): {0:?}")]
    ServiceManagerValidateUnknown(alloy_contract::Error),

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

    #[error("Failed to encode with bincode: {0:?}")]
    BincodeEncode(#[from] bincode::error::EncodeError),

    #[error("Failed to decode with bincode: {0:?}")]
    BincodeDecode(#[from] bincode::error::DecodeError),

    #[error("Unable to fetch service: {0:?}")]
    FetchService(anyhow::Error),

    #[error("Unable to look up operator key from signing key: {0:?}")]
    OperatorKeyLookup(alloy_contract::Error),
}

#[derive(Error, Debug)]
pub enum PacketValidationError {
    #[error("Unexpected envelope difference")]
    EnvelopeDiff,
}
