#[allow(warnings)]
mod bindings;
use bindings::{Contract, Guest};
use example_helpers::trigger::{decode_trigger_input, encode_trigger_output};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let (trigger_id, input) = decode_trigger_input(input)?;

        let Request { x } = serde_json::from_slice(&input)
            .map_err(|e| format!("Could not deserialize input request from JSON: {:?}", e))?;

        let y = x * x;

        let data = serde_json::to_vec(&Response { y })
            .map_err(|e| format!("Could not serialize output data into JSON: {:?}", e))?;

        Ok(encode_trigger_output(trigger_id, data))
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
