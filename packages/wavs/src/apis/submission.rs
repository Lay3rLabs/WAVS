use thiserror::Error;
use tokio::sync::mpsc;
use wavs_types::{Submit, TriggerConfig};

use crate::AppContext;

pub trait Submission: Send + Sync {
    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(
        &self,
        ctx: AppContext,
        receiver: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError>;

    fn add_service(&self, service: &wavs_types::Service) -> Result<(), SubmissionError>;
}

/// The data returned from a trigger action
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainMessage {
    pub trigger_config: TriggerConfig,
    pub wasi_result: Vec<u8>,
    pub submit: Submit,
}

#[derive(Error, Debug)]
pub enum SubmissionError {
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
    #[error("failed to sign payload")]
    FailedToSignPayload,
    #[error("failed to submit to eth directly: {0}")]
    FailedToSubmitEthDirect(anyhow::Error),
    #[error("failed to submit to cosmos: {0}")]
    FailedToSubmitCosmos(anyhow::Error),
    #[error("missing service handler index")]
    MissingServiceHandlerIndex,
}
