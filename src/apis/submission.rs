use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::{runtime::Runtime, sync::mpsc};

use super::ID;

pub trait Submission {
    /// Start running the trigger manager.
    /// This can create it's own default runtime or use the runtime passed in.
    /// This should only be called once in the lifetime of the object.
    fn start(&self, rt: Option<Arc<Runtime>>, input: mpsc::Receiver<ChainMessage>);
}

/// The data returned from a trigger action
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChainMessage {
    /// Identify which service and workflow this came from
    pub service_id: ID,
    pub workflow_id: ID,

    pub task_id: u64,
    pub wasm_result: Vec<u8>,
    pub hd_index: u32,
    pub verifier_addr: String,
}
