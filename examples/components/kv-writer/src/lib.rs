use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, Guest, TriggerAction},
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};
use example_helpers::bindings::world::wasi;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Writer component triggered");

        let trigger_id = 1u64;
        
        let test_input = SquareRequest { x: 10 };

        // Open the keyvalue store using generated bindings
        let bucket = wasi::keyvalue::store::open("").map_err(|e| format!("Failed to open store: {:?}", e))?;

        // Save the input data
        let data = serde_json::to_string(&test_input).map_err(|e| e.to_string())?;
        bucket
            .set("square_input", data.as_bytes())
            .map_err(|e| format!("Failed to store data: {:?}", e))?;

        host::log(
            host::LogLevel::Info,
            &format!("Saved square input: x={}", test_input.x),
        );

        let response = Response { saved_x: test_input.x };
        let resp = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        
        Ok(Some(encode_trigger_output(trigger_id, resp)))
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SquareRequest {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub saved_x: u64,
}

export_layer_trigger_world!(Component);