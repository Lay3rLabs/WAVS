use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;
use utils::error::EthClientError;
use wavs_types::{ChainName, Envelope, PacketRoute, Service, ServiceID, Submit};

use crate::AppContext;

#[async_trait]
pub trait Submission: Send + Sync {
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
    #[error("eth client: {0}")]
    EthClient(#[from] EthClientError),
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
    #[error("ethereum: {0}")]
    Ethereum(anyhow::Error),
    #[error("missing ethereum chain")]
    MissingEthereumChain,
    #[error("chain is not an ethereum chain")]
    NotEthereumChain,
    #[error("cross-chain submissions are not supported yet")]
    NoCrossChainSubmissions,
    #[error("missing aggregator endpoint")]
    MissingAggregatorEndpoint,
    #[error("aggregator url: {0}")]
    AggregatorUrl(url::ParseError),
    #[error("cosmos parse: {0}")]
    CosmosParse(anyhow::Error),
    #[error("expected eth address, got: {0}")]
    ExpectedEthAddress(String),
    #[error("expected eth message")]
    ExpectedEthMessage,
    #[error("failed to sign envelope: {0:?}")]
    FailedToSignEnvelope(alloy_signer::Error),
    #[error("failed to submit to eth directly: {0}")]
    FailedToSubmitEthDirect(anyhow::Error),
    #[error("failed to submit to cosmos: {0}")]
    FailedToSubmitCosmos(anyhow::Error),
    #[error("missing ethereum signer for service {0}")]
    MissingEthereumSigner(ServiceID),
    #[error("failed to create signer for service {0}: {1:?}")]
    FailedToCreateEthereumSigner(ServiceID, anyhow::Error),
    #[error("missing ethereum signing client for chain {0}")]
    MissingEthereumSendingClient(ChainName),
}
