use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, Guest, TriggerAction},
    export_layer_trigger_world,
};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse, KvStoreResult};

// use example_helpers::bindings::world::wasi::keyvalue::store;

struct Component;

// TODO - bring back `bindings::world::store` and implement the component below with keyvalue store

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Store component triggered");

        let (trigger_id, req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        let resp = match serde_json::from_slice::<KvStoreRequest>(&req) {
            Ok(KvStoreRequest::Write {
                key,
                value,
                read_immediately,
            }) => {
                write_value(&key, value).map_err(|e| e.to_string())?;
                match read_immediately {
                    true => {
                        let value = read_value(&key).map_err(|e| e.to_string())?;
                        KvStoreResponse::Read { value }
                    }
                    false => KvStoreResponse::Write,
                }
            }
            Ok(KvStoreRequest::Read { key }) => {
                let value = read_value(&key).map_err(|e| e.to_string())?;
                KvStoreResponse::Read { value }
            }
            Err(e) => {
                return Err(format!("Failed to parse request: {e}"));
            }
        };

        let resp_bytes =
            serde_json::to_vec(&resp).map_err(|e| format!("Failed to serialize response: {e}"))?;

        Ok(Some(encode_trigger_output(trigger_id, resp_bytes)))
    }
}

fn read_value(key: &str) -> KvStoreResult<Vec<u8>> {
    // TODO - bring back `store`
    // for right now, just using local file system to simulate key-value store

    let storage_path = Path::new("./keyvalue").join(format!("{key}.txt"));
    if !storage_path.exists() {
        return Err(KvStoreError::KeyNotFound(key.to_string()));
    }

    let mut storage_file = fs::File::open(&storage_path)?;

    let mut value = Vec::new();
    storage_file.read_to_end(&mut value)?;

    Ok(value)

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

fn write_value(key: &str, value: Vec<u8>) -> KvStoreResult<()> {
    // TODO - bring back `store`
    // for right now, just using local file system to simulate key-value store
    let storage_path = Path::new("./keyvalue");
    if !storage_path.exists() {
        fs::create_dir_all(storage_path)?;
    }

    let storage_path = storage_path.join(format!("{key}.txt"));
    let mut storage_file = fs::File::create(&storage_path)?;

    storage_file.write_all(&value)?;

    Ok(())

    // let bucket = store::open("default").map_err(|e| format!("Failed to open bucket: {:?}", e))?;
    // bucket
    //     .set(key, value.to_string().as_bytes())
    //     .map_err(|e| format!("Failed to set key {}: {:?}", key, e))
}

export_layer_trigger_world!(Component);
