use thiserror::Error;

#[derive(Debug, Error)]
pub enum EthClientError {
    #[error("Missing mnemonic")]
    MissingMnemonic,

    #[error("No Transaction Receipt")]
    NoTransactionReceipt,
}
