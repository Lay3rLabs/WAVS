use lavs_apis::id::TaskId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::context::AppContext;

use super::trigger::TriggerData;

pub trait Submission: Send + Sync {
    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(&self, ctx: AppContext) -> Result<mpsc::Sender<ChainMessage>, SubmissionError>;
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChainMessage {
    /// Identify which trigger this came from
    pub trigger_data: TriggerData,

    pub task_id: TaskId,
    pub wasm_result: Vec<u8>,
    pub hd_index: u32,
    pub verifier_addr: String,
}

#[derive(Error, Debug)]
pub enum SubmissionError {
    #[error("climb: {0}")]
    Climb(anyhow::Error),
    #[error("missing mnemonic")]
    MissingMnemonic,
}
