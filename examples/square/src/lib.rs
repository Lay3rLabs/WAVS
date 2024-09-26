#[allow(warnings)]
mod bindings;

use anyhow::anyhow;
use bindings::Guest;
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
    fn handle_update() -> Result<(), String> {
        Ok(())
    }

    fn run(_timestamp: u64, json_input: String) -> Result<String, String> {
        let TaskRequestData { x } = serde_json::from_str(&json_input)
            .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
            .unwrap();
        let y = x * x;
        println!("{}^2 = {}", x, y);

        serde_json::to_string(&TaskResponseData { y })
            .map_err(|e| format!("Could not serialize output data into JSON: {e}"))
    }
}

bindings::export!(Component with_types_in bindings);
