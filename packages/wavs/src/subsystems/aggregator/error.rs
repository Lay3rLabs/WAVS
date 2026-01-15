use thiserror::Error;
use utils::error::EvmClientError;
use wavs_types::{
    contracts::cosmwasm::service_manager::error::WavsValidateError, ChainConfigError,
    QuorumQueueId, ServiceManagerError, SigningError,
};

use crate::{dispatcher::DispatcherCommand, subsystems::aggregator::AggregatorCommand};

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("DB: {0}")]
    Db(anyhow::Error),

    #[error("EVM client: {0}")]
    EvmClient(#[from] EvmClientError),

    #[error("Cosmos Client: {0}")]
    CosmosClient(anyhow::Error),

    #[error("Aggregator channel error: {0}")]
    SendCommandToSelf(#[from] Box<crossbeam::channel::SendError<AggregatorCommand>>),

    #[error("Aggregator channel error: {0}")]
    SendCommandToDispatcher(#[from] Box<crossbeam::channel::SendError<DispatcherCommand>>),

    #[error("Chain Config: {0}")]
    Config(#[from] ChainConfigError),

    #[error("Evm client create: {0:?}")]
    CreateEvmClient(anyhow::Error),

    #[error("Service manager validate(): {0:?}")]
    CosmosServiceManagerValidate(WavsValidateError),

    #[error("Service manager validate(): {0:?}")]
    EvmServiceManagerValidateKnown(ServiceManagerError),

    #[error("Service manager validate(): {0}")]
    EvmServiceManagerValidateAnyRevert(String),

    #[error("Service manager validate(): {0:?}")]
    EvmServiceManagerValidateUnknown(alloy_contract::Error),

    #[error("Insufficient Quorum: weight {signer_weight} < threshold {threshold_weight} (total weight: {total_weight})")]
    InsufficientQuorum {
        signer_weight: String,
        threshold_weight: String,
        total_weight: String,
    },

    #[error("Unable to look up service manager from service handler: {0:?}")]
    EvmServiceManagerLookup(alloy_contract::Error),

    #[error("Block number: {0}")]
    BlockNumber(anyhow::Error),

    #[error("Join: {0}")]
    JoinError(String),

    #[error("Queue is empty: {0:?}")]
    QueueIsEmpty(QuorumQueueId),

    #[error("Signing: {0:?}")]
    Signing(#[from] SigningError),

    #[error("Unexpected envelope difference for queue {0:?}")]
    EnvelopeDiff(QuorumQueueId),

    #[error("P2P: {0}")]
    P2p(String),
}
