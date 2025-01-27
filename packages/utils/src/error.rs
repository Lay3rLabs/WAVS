use alloy::primitives::Address;
use thiserror::Error;

use crate::types::ChainName;

#[derive(Debug, Error)]
pub enum EthClientError {
    #[error("Missing mnemonic")]
    MissingMnemonic,

    #[error("No Transaction Receipt")]
    NoTransactionReceipt,

    #[error("Contract not deployed {0}")]
    ContractNotDeployed(Address),
}

#[derive(Debug, Error)]
pub enum ChainConfigError {
    #[error("Expected Ethereum chain")]
    ExpectedEthChain,

    #[error("Expected Cosmos chain")]
    ExpectedCosmosChain,

    #[error("Duplidate chain name for {0}")]
    DuplicateChainName(ChainName),
}
