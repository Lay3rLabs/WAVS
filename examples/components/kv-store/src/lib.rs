use example_helpers::bindings::world::wasi::keyvalue::store;
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
            Ok(KvStoreRequest::Write { key, value }) => {
                write_value(&key, &value).map_err(|e| e.to_string())?;
                KvStoreResponse::Write
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

fn open_bucket() -> KvStoreResult<store::Bucket> {
    store::open("default").map_err(|e| KvStoreError::StoreBucketOpen(e.to_string()))
}

fn read_value(key: &str) -> KvStoreResult<Vec<u8>> {
    let bucket = open_bucket()?;
    bucket
        .get(key)
        .map_err(|e| KvStoreError::StoreReadKey(e.to_string()))?
        .ok_or_else(|| KvStoreError::MissingKey {
            key: key.to_string(),
        })
}

fn write_value(key: &str, value: &[u8]) -> KvStoreResult<()> {
    let bucket = open_bucket()?;
    bucket
        .set(key, value)
        .map_err(|e| KvStoreError::StoreWriteKey(e.to_string()))?;

    Ok(())
}

export_layer_trigger_world!(Component);
