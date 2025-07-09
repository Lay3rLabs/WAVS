use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SquareRequest {
    pub x: u64,
}

impl SquareRequest {
    pub fn new(x: u64) -> Self {
        SquareRequest { x }
    }
}

impl SquareRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SquareResponse {
    pub y: u64,
}

impl SquareResponse {
    pub fn new(y: u64) -> Self {
        SquareResponse { y }
    }
}
