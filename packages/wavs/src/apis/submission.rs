use thiserror::Error;
use tokio::sync::mpsc;
use utils::layer_contract_client::TriggerId;

use crate::AppContext;

use super::{dispatcher::Submit, trigger::TriggerConfig};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainMessage {
    Cosmos {
        trigger_config: TriggerConfig,
        wasm_result: Vec<u8>,
        submit: Submit,
    },
    Eth {
        trigger_config: TriggerConfig,
        wasm_result: Vec<u8>,
        trigger_id: TriggerId,
        submit: Submit,
    },
}

impl ChainMessage {
    pub fn wasm_result(&self) -> &[u8] {
        match self {
            ChainMessage::Cosmos { wasm_result, .. } => wasm_result,
            ChainMessage::Eth { wasm_result, .. } => wasm_result,
        }
    }

    pub fn submit(&self) -> &Submit {
        match self {
            ChainMessage::Cosmos { submit, .. } => submit,
            ChainMessage::Eth { submit, .. } => submit,
        }
    }

    pub fn trigger_config(&self) -> &TriggerConfig {
        match self {
            ChainMessage::Cosmos { trigger_config, .. } => trigger_config,
            ChainMessage::Eth { trigger_config, .. } => trigger_config,
        }
    }
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
    #[error("expected eth address, got: {0}")]
    ExpectedEthAddress(String),
    #[error("expected eth message")]
    ExpectedEthMessage,
    #[error("failed to sign payload")]
    FailedToSignPayload,
    #[error("failed to submit to eth directly: {0}")]
    FailedToSubmitEthDirect(anyhow::Error),
}
