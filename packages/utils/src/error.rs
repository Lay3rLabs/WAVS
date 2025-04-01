use alloy::rpc::types::TransactionReceipt;
use thiserror::Error;

use wavs_types::ChainName;

#[derive(Debug, Error)]
pub enum EthClientError {
    #[error("Missing mnemonic")]
    MissingMnemonic,

    #[error("Contract not deployed {0}")]
    ContractNotDeployed(alloy::primitives::Address),

    #[error("No Transaction Receipt: {0}")]
    TransactionWithoutReceipt(anyhow::Error),

    #[error("Transaction Receipt: {0:#?}")]
    TransactionWithReceipt(Box<TransactionReceipt>),

    #[error("Unable to sign: {0:#?}")]
    Signing(anyhow::Error),
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
