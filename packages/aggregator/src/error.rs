use thiserror::Error;
use utils::{error::EvmClientError, storage::db::DBError};
use wavs_types::{
    contracts::cosmwasm::service_manager::error::WavsValidateError, ChainConfigError, ChainName,
    EnvelopeError, ServiceID, ServiceManagerError, WorkflowID,
};

pub type AggregatorResult<T> = Result<T, AggregatorError>;

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("Missing workflow: {workflow_id} for service: {service_id}")]
    MissingWorkflow {
        workflow_id: WorkflowID,
        service_id: ServiceID,
    },

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
    ServiceManagerValidateUnknownEvm(alloy_contract::Error),

    #[error("Service manager validate(): {0:?}")]
    ServiceManagerValidateUnknownCosmos(anyhow::Error),

    #[error("Service manager validate(): {0:?}")]
    ServiceManagerValidateWavs(WavsValidateError),

    #[error("Chain not found: {0}")]
    ChainNotFound(ChainName),

    #[error("Missing EVM credential")]
    MissingEvmCredential,

    #[error("Missing Cosmos credential")]
    MissingCosmosCredential,

    #[error("Corrupt Cosmos credential: {0:?}")]
    CorruptCosmosCredential(anyhow::Error),

    #[error("Unable to create cosmos client: {0:?}")]
    CreateCosmosClient(anyhow::Error),

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
    OperatorKeyLookupEvm(alloy_contract::Error),

    #[error("Unable to look up operator key from signing key: {0:?}")]
    OperatorKeyLookupCosmos(anyhow::Error),

    #[error("Unable to look up service manager from evm service handler: {0:?}")]
    EvmServiceManagerLookup(alloy_contract::Error),

    #[error("Unable to look up service manager from cosmos service handler: {0:?}")]
    CosmosServiceManagerLookup(anyhow::Error),

    #[error("Service already registered: {0}")]
    RepeatService(ServiceID),

    #[error("No such service registered: {0}")]
    MissingService(ServiceID),

    #[error("service handler could not handle signed envelope: {0}")]
    CosmosHandleSignedEnvelope(anyhow::Error),

    #[error("deadpool: {0:?}")]
    Deadpool(deadpool::managed::PoolError<anyhow::Error>),
}

#[derive(Error, Debug)]
pub enum PacketValidationError {
    #[error("Unexpected envelope difference")]
    EnvelopeDiff,
}
