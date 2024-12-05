use lavs_apis::id::TaskId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::context::AppContext;

use super::{dispatcher::Submit, trigger::TriggerData};

pub trait Submission: Send + Sync {
    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(
        &self,
        ctx: AppContext,
        receiver: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError>;
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChainMessage {
    /// Identify which trigger this came from
    pub trigger_data: TriggerData,
    pub task_id: TaskId,
    pub wasm_result: Vec<u8>,
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
    #[error("faucet: {0}")]
    Faucet(String),
    #[error("missing layer chain")]
    MissingLayerChain,
    #[error("ethereum: {0}")]
    Ethereum(anyhow::Error),
    #[error("missing ethereum chain")]
    MissingEthereumChain,
    #[error("cross-chain submissions are not supported yet")]
    NoCrossChainSubmissions,
}
