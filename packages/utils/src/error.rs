use alloy_rpc_types_eth::TransactionReceipt;
use thiserror::Error;

use wavs_types::ChainName;

#[derive(Debug, Error)]
pub enum EvmClientError {
    #[error("HD index must be zero when using a private key (use mnemonic instead)")]
    DerivationWithPrivateKey,

    #[error("Contract not deployed {0}")]
    ContractNotDeployed(alloy_primitives::Address),

    #[error("No Transaction Receipt: {0}")]
    TransactionWithoutReceipt(anyhow::Error),

    #[error("Transaction Receipt: {0:#?}")]
    TransactionWithReceipt(Box<TransactionReceipt>),

    #[error("Unable to sign: {0:#?}")]
    Signing(anyhow::Error),

    #[error("Unable to estimate gas: {0:#?}")]
    GasEstimation(anyhow::Error),

    #[error("Unable to recover signer address: {0:#?}")]
    RecoverSignerAddress(anyhow::Error),

    #[error("Unable to parse endpoint: {0}")]
    ParseEndpoint(String),

    #[error("Unable to create web socket provider: {0:#?}")]
    WebSocketProvider(anyhow::Error),

    #[error("Unable to create http provider: {0:#?}")]
    HttpProvider(anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ChainConfigError {
    #[error("Expected EVM chain")]
    ExpectedEvmChain,

    #[error("Expected Cosmos chain")]
    ExpectedCosmosChain,

    #[error("Duplicate chain name for {0}")]
    DuplicateChainName(ChainName),
}
