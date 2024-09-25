#[allow(warnings)]
mod bindings;

use anyhow::anyhow;
use bindings::{Guest, Input};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct TaskRequestData {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct TaskResponseData {
    pub y: u64,
}
struct Component;

impl Guest for Component {
    fn handle_upgrade() -> Result<(), String> {
        Ok(())
    }

    fn run_task(input: Input) -> Result<Vec<u8>, String> {
        let TaskRequestData { x } = serde_json::from_slice(&input.bytes)
            .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
            .unwrap();
        let y = x * x;
        println!("{}^2 = {}", x, y);

        Ok(serde_json::to_vec(&TaskResponseData { y })
            .map_err(|e| anyhow!("Could not serialize output data into JSON: {}", e))
            .unwrap())
    }
}

bindings::export!(Component with_types_in bindings);
