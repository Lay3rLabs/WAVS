use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;
use utils::error::EvmClientError;
use wavs_types::{ChainName, Envelope, EnvelopeError, PacketRoute, Service, ServiceID, Submit};

use crate::AppContext;

#[async_trait]
pub trait Submission: Send + Sync {
    const CHANNEL_SIZE: usize = 20;

    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(
        &self,
        ctx: AppContext,
        receiver: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError>;

    async fn add_service(&self, service: &Service) -> Result<(), SubmissionError>;

    fn remove_service(&self, service_id: ServiceID) -> Result<(), SubmissionError>;

    fn get_service_key(
        &self,
        service_id: ServiceID,
    ) -> Result<wavs_types::SigningKeyResponse, SubmissionError>;
}

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub packet_route: PacketRoute,
    pub envelope: Envelope,
    pub submit: Submit,
}

#[derive(Error, Debug)]
pub enum SubmissionError {
    #[error("EVM client: {0}")]
    EvmClient(#[from] EvmClientError),
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("missing mnemonic")]
    MissingMnemonic,
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
    #[error("missing EVM chain")]
    MissingEvmChain,
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
    FailedToSignEnvelope(alloy_signer::Error),
    #[error("failed to submit to EVM directly: {0}")]
    FailedToSubmitEvmDirect(anyhow::Error),
    #[error("failed to submit to cosmos: {0}")]
    FailedToSubmitCosmos(anyhow::Error),
    #[error("missing EVM signer for service {0}")]
    MissingEvmSigner(ServiceID),
    #[error("failed to create EVM signer for service {0}: {1:?}")]
    FailedToCreateEvmSigner(ServiceID, anyhow::Error),
    #[error("missing EVM signing client for chain {0}")]
    MissingEvmSendingClient(ChainName),
    #[error("envelope {0:?}")]
    Envelope(#[from] EnvelopeError),
}
