use thiserror::Error;
use utils::error::EvmClientError;

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("EVM client: {0}")]
    EvmClient(#[from] EvmClientError),
}
