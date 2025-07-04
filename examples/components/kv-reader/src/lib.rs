use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, wasi::keyvalue::store, Guest, TriggerAction},
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Reader component triggered");

        let (trigger_id, _req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        // Try to read the saved data using wasi:keyvalue
        let bucket =
            store::open("default").map_err(|e| format!("Failed to open bucket: {:?}", e))?;
        match bucket
            .get("square_input")
            .map_err(|e| format!("Failed to get data: {:?}", e))?
        {
            Some(bytes) => {
                let data_str = String::from_utf8_lossy(&bytes);
                let square_data: SquareRequest = serde_json::from_str(&data_str)
                    .map_err(|e| format!("Failed to deserialize saved data: {}", e))?;

                host::log(
                    host::LogLevel::Info,
                    &format!("Read square input from store: x={}", square_data.x),
                );

                let response = Response {
                    read_x: square_data.x,
                };
                let resp = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
                Ok(Some(encode_trigger_output(trigger_id, resp)))
            }
            None => Err("No data found in keyvalue store".to_string()),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SquareRequest {
    pub x: u64,
}

#[derive(Serialize, Debug)]
pub struct Response {
    pub read_x: u64,
}

export_layer_trigger_world!(Component);
