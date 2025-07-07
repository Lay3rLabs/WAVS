use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, Guest, TriggerAction},
    export_layer_trigger_world,
};
use serde::Serialize;

// use example_helpers::bindings::world::wasi::keyvalue::store;

struct Component;

// TODO - bring back `bindings::world::store` and implement the component below with keyvalue store

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Store component triggered");

        let (trigger_id, _req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        // Read current counter value, defaulting to 0 if not found
        let current_counter = read_value("counter")?.unwrap_or(0);

        // Increment counter
        let new_counter = current_counter + 1;

        // Save the new counter value
        write_value("counter", new_counter)?;

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

fn read_value(key: &str) -> Result<Option<u64>, String> {
    // TODO - bring back `store`
    // for right now, just using local file system to simulate key-value store

    let storage_path = Path::new("./keyvalue").join(format!("{}.txt", key));
    if !storage_path.exists() {
        return Ok(None);
    }

    let mut storage_file = fs::File::open(&storage_path).map_err(|e| e.to_string())?;
    let mut buffer = [0; 8]; // u64 is 8 bytes
    storage_file
        .read_exact(&mut buffer)
        .map_err(|e| format!("Failed to read from storage file: {}", e))?;

    let value = u64::from_be_bytes(buffer);
    Ok(Some(value))

    // let bucket = store::open("default").map_err(|e| format!("Failed to open bucket: {:?}", e))?;
    // match bucket.get(key).map_err(|e| format!("Failed to get key {}: {:?}", key, e))? {
    //     Some(bytes) => {
    //         let value_str = String::from_utf8(bytes)
    //             .map_err(|e| format!("Failed to parse bytes for key {}: {}", key, e))?;
    //         value_str
    //             .parse::<u64>()
    //             .map_err(|e| format!("Failed to parse value for key {}: {}", key, e))
    //     }
    //     None => Ok(0), // Default value if key not found
    // }
}

fn write_value(key: &str, value: u64) -> Result<(), String> {
    // TODO - bring back `store`
    // for right now, just using local file system to simulate key-value store
    let storage_path = Path::new("./keyvalue");
    if !storage_path.exists() {
        fs::create_dir_all(storage_path).map_err(|e| e.to_string())?;
    }

    let storage_path = storage_path.join(&format!("{}.txt", key));
    let mut storage_file = fs::File::create(&storage_path).map_err(|e| e.to_string())?;

    storage_file
        .write_all(&value.to_be_bytes())
        .map_err(|e| e.to_string())?;

    Ok(())

    // let bucket = store::open("default").map_err(|e| format!("Failed to open bucket: {:?}", e))?;
    // bucket
    //     .set(key, value.to_string().as_bytes())
    //     .map_err(|e| format!("Failed to set key {}: {:?}", key, e))
}

#[derive(Serialize, Debug)]
pub struct KvStoreResponse {
    pub previous_value: u64,
    pub new_value: u64,
}

export_layer_trigger_world!(Component);
