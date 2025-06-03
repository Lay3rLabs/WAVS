use serde::{Deserialize, Serialize};

pub const COMPONENT_SQUARE: &[u8] =
    include_bytes!("../../../../examples/build/components/square.wasm");
pub const COMPONENT_ECHO_DATA: &[u8] =
    include_bytes!("../../../../examples/build/components/echo_data.wasm");
pub const COMPONENT_PERMISSIONS: &[u8] =
    include_bytes!("../../../../examples/build/components/permissions.wasm");

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct SquareIn {
    pub x: u64,
}

impl SquareIn {
    pub fn new(x: u64) -> Self {
        SquareIn { x }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]

pub struct SquareOut {
    pub y: u64,
}

impl SquareOut {
    pub fn new(y: u64) -> Self {
        SquareOut { y }
    }
}
