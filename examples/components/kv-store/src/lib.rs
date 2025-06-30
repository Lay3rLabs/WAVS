use example_helpers::bindings::world::{host, Guest, TriggerAction, WasmResponse};
use example_helpers::export_layer_trigger_world;

wit_bindgen::generate!({
    world: "layer-trigger-world",
    path: "../../../wit",
    additional_derives: [serde::Deserialize, serde::Serialize],
    generate_all,
});

use wasi::keyvalue::store::{open, get, set};

struct Counter;

impl Guest for Counter {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "Counter component triggered");

        // Open the keyvalue store
        let store = open("").map_err(|e| format!("Failed to open store: {:?}", e))?;

        let current_count = match get(&store, "counter") {
            Ok(Some(bytes)) => {
                let count_str = String::from_utf8_lossy(&bytes);
                count_str.parse::<u32>().unwrap_or(0)
            }
            _ => 0,
        };

        let new_count = current_count + 1;

        // Store the new count
        set(&store, "counter", new_count.to_string().as_bytes().to_vec())
            .map_err(|e| format!("Failed to store counter: {:?}", e))?;

        host::log(
            host::LogLevel::Info,
            &format!(
                "Counter incremented from {} to {}",
                current_count, new_count
            ),
        );

        Ok(Some(WasmResponse {
            payload: format!("count:{}", new_count).as_bytes().to_vec(),
            ordering: None,
        }))
    }
}

export_layer_trigger_world!(Counter);
