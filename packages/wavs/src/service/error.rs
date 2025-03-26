use thiserror::Error;
use wavs_types::ChainName;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Download: {0}")]
    Download(anyhow::Error),

    #[error("Missing Ethereum Chain: {0}")]
    MissingEthereumChain(ChainName),

    #[error("Ethereum: {0}")]
    Ethereum(#[from] anyhow::Error),

    #[error("Service Metadata Fetch Error: {0}")]
    ServiceMetadataFetchError(String),
}
