use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::AppContext;

use super::{
    dispatcher::{Submit, SubmitFormat},
    trigger::TriggerAction,
};

pub trait Submission: Send + Sync {
    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(
        &self,
        ctx: AppContext,
        receiver: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError>;
}

/// The data passed to the submission manager
/// constructed from, basically, trigger + engine result
#[derive(Debug)]
pub struct ChainMessage {
    pub trigger: TriggerAction,
    pub wasm_result: Vec<u8>,
    pub submit: Submit,
    pub submit_format: SubmitFormat,
}

/// For usecases where we need to sign additional data besides the raw output
/// it gets set in this wrapper, and then _that_ becomes the submission data
/// SubmitFormat::Raw is not used in this case, it's just sent directly
#[derive(Serialize, Deserialize, Debug)]
pub struct SubmitWrapper {
    // set if SubmitFormat is InputOutputId
    pub input: Option<Vec<u8>>,
    // set if SubmitFormat is InputOutputId or OutputId
    pub id: Option<String>,
    pub data: Vec<u8>,
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
    #[error("mismatched trigger / submit format")]
    MismatchTriggerFormat,
    #[error("serde: {0}")]
    Serde(serde_json::Error),
}
