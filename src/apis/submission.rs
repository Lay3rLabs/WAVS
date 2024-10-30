use lavs_apis::id::TaskId;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::context::AppContext;

use super::ID;

pub trait Submission: Send + Sync {
    /// Start running the submission manager
    /// This should only be called once in the lifetime of the object.
    fn start(&self, ctx: AppContext) -> Result<mpsc::Sender<ChainMessage>, SubmissionError>;
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChainMessage {
    /// Identify which service and workflow this came from
    pub service_id: ID,
    pub workflow_id: ID,

    pub task_id: TaskId,
    pub wasm_result: Vec<u8>,
    pub hd_index: u32,
    pub verifier_addr: String,
}

#[derive(Error, Debug)]
pub enum SubmissionError {
    #[error("chain error: {0}")]
    ChainError(anyhow::Error),
}
