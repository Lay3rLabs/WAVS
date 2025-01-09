use serde::{Deserialize, Serialize};

// Submission Contracts must have an execute handler with at least this variant (ofc it can have more)
#[derive(Deserialize, Serialize, Debug)]
pub enum LayerExecuteMsg {
    LayerSubmit { data: Vec<u8>, signature: Vec<u8> },
}

impl LayerExecuteMsg {
    pub fn new(data: Vec<u8>, signature: Vec<u8>) -> Self {
        LayerExecuteMsg::LayerSubmit { data, signature }
    }
}
