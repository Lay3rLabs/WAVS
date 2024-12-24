use alloy::primitives::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EthClientError {
    #[error("Missing mnemonic")]
    MissingMnemonic,

    #[error("No Transaction Receipt")]
    NoTransactionReceipt,

    #[error("Contract not deployed {0}")]
    ContractNotDeployed(Address),

    #[error("Chain not found")]
    ChainNotFound,
}
