use thiserror::Error;
use utils::error::EvmClientError;
use wavs_types::{ChainKey, EnvelopeError, ServiceId, Submit};

use crate::services::ServicesError;

#[derive(Error, Debug)]
pub enum SubmissionError {
    #[error("EVM client: {0}")]
    EvmClient(#[from] EvmClientError),
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("missing mnemonic")]
    MissingMnemonic,
    #[error("missing key for service {service_id}")]
    MissingServiceKey { service_id: ServiceId },
    #[error("faucet url: {0}")]
    FaucetUrl(url::ParseError),
    #[error("reqwest: {0}")]
    Reqwest(reqwest::Error),
    #[error("aggregator: {0}")]
    Aggregator(String),
    #[error("faucet: {0}")]
    Faucet(String),
    #[error("missing cosmos chain")]
    MissingCosmosChain,
    #[error("evm: {0}")]
    EVM(anyhow::Error),
    #[error("missing EVM chain for {0}")]
    MissingEvmChain(ChainKey),
    #[error("chain is not an EVM chain")]
    NotEvmChain,
    #[error("cross-chain submissions are not supported yet")]
    NoCrossChainSubmissions,
    #[error("missing aggregator endpoint")]
    MissingAggregatorEndpoint,
    #[error("aggregator url: {0}")]
    AggregatorUrl(url::ParseError),
    #[error("cosmos parse: {0}")]
    CosmosParse(anyhow::Error),
    #[error("expected EVM address, got: {0}")]
    ExpectedEvmAddress(String),
    #[error("expected EVM message")]
    ExpectedEvmMessage,
    #[error("failed to sign envelope: {0:?}")]
    FailedToSignEnvelope(anyhow::Error),
    #[error("failed to submit to EVM directly: {0}")]
    FailedToSubmitEvmDirect(anyhow::Error),
    #[error("failed to submit to cosmos: {0}")]
    FailedToSubmitCosmos(anyhow::Error),
    #[error("missing EVM signer for service {0}")]
    MissingEvmSigner(ServiceId),
    #[error("failed to create EVM signer for service {0}: {1:?}")]
    FailedToCreateEvmSigner(ServiceId, anyhow::Error),
    #[error("missing EVM signing client for chain {0}")]
    MissingEvmSendingClient(ChainKey),
    #[error("envelope {0:?}")]
    Envelope(#[from] EnvelopeError),
    #[error("services {0:?}")]
    Services(#[from] ServicesError),
    #[error("Cannot submit to {0:?}")]
    InvalidSubmitKind(Submit),
}
