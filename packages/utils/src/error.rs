use alloy_rpc_types_eth::TransactionReceipt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmClientError {
    #[error("HD index must be zero when using a private key (use mnemonic instead)")]
    DerivationWithPrivateKey,

    #[error("Contract not deployed {0}")]
    ContractNotDeployed(alloy_primitives::Address),

    #[error("Address is not a contract: {0}")]
    NotContract(alloy_primitives::Address),

    #[error("Could not get contract code at {0}: {1:?}")]
    FailedGetCode(alloy_primitives::Address, anyhow::Error),

    #[error("Send Transaction Error: {0}")]
    SendTransaction(anyhow::Error),

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

    #[error("Unable to get block height")]
    BlockHeight,
}

#[derive(Debug, Error)]
pub enum SvmClientError {
    #[error("Unable to parse endpoint: {0}")]
    ParseEndpoint(String),

    #[error("Unable to create WebSocket client: {0:#?}")]
    WebSocketClient(anyhow::Error),

    #[error("Unable to subscribe to program logs: {0:#?}")]
    ProgramLogsSubscription(anyhow::Error),

    #[error("Invalid program ID: {0}")]
    InvalidProgramId(String),

    #[error("Connection failed: {0:#?}")]
    ConnectionFailed(anyhow::Error),
}
