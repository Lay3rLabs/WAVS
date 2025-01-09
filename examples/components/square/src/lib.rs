#[allow(warnings)]
mod bindings;
use anyhow::anyhow;
use bindings::{Contract, Guest};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let Request { x } = serde_json::from_slice(&input)
            .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
            .unwrap();

        let y = x * x;

        println!("{}^2 = {}", x, y);

        Ok(serde_json::to_vec(&Response { y })
            .map_err(|e| anyhow!("Could not serialize output data into JSON: {}", e))
            .unwrap())
    }
}

bindings::export!(Component with_types_in bindings);

#[derive(Deserialize, Debug)]
pub struct Request {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub y: u64,
}
