use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, wasi::keyvalue::store, Guest, TriggerAction},
    export_layer_trigger_world,
};
use serde::Serialize;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Store component triggered");

        let (trigger_id, _req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        // Open the keyvalue store
        let bucket =
            store::open("default").map_err(|e| format!("Failed to open bucket: {:?}", e))?;

        // Read current counter value, defaulting to 0 if not found
        let current_counter = match bucket
            .get("counter")
            .map_err(|e| format!("Failed to get counter: {:?}", e))?
        {
            Some(bytes) => {
                let counter_str = String::from_utf8(bytes)
                    .map_err(|e| format!("Failed to parse counter bytes: {}", e))?;
                counter_str
                    .parse::<u64>()
                    .map_err(|e| format!("Failed to parse counter value: {}", e))?
            }
            None => 0,
        };

        // Increment counter
        let new_counter = current_counter + 1;

        // Save the new counter value
        bucket
            .set("counter", new_counter.to_string().as_bytes())
            .map_err(|e| format!("Failed to store counter: {:?}", e))?;

        host::log(
            host::LogLevel::Info,
            &format!(
                "KV Store counter incremented from {} to {}",
                current_counter, new_counter
            ),
        );

        let response = KvStoreResponse {
            previous_value: current_counter,
            new_value: new_counter,
        };
        let resp = serde_json::to_vec(&response).map_err(|e| e.to_string())?;

        Ok(Some(encode_trigger_output(trigger_id, resp)))
    }
}

#[derive(Serialize, Debug)]
pub struct KvStoreResponse {
    pub previous_value: u64,
    pub new_value: u64,
}

export_layer_trigger_world!(Component);
