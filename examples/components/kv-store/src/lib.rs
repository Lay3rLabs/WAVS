use std::collections::HashMap;

use example_helpers::bindings::world::wasi::keyvalue::store::KeyResponse;
use example_helpers::bindings::world::wasi::keyvalue::{atomics, batch, store};
use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, Guest, TriggerAction},
    export_layer_trigger_world,
};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse, KvStoreResult};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "KV Store component triggered");

        let (trigger_id, req) =
            decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

        let resp = match serde_json::from_slice::<KvStoreRequest>(&req) {
            Ok(KvStoreRequest::Write { bucket, key, value }) => {
                write_value(&bucket, &key, &value).map_err(|e| e.to_string())?;
                KvStoreResponse::Write
            }
            Ok(KvStoreRequest::Read { bucket, key }) => {
                let value = read_value(&bucket, &key).map_err(|e| e.to_string())?;
                KvStoreResponse::Read { value }
            }
            Ok(KvStoreRequest::AtomicIncrement { bucket, key, delta }) => {
                let value = atomic_increment(&bucket, &key, delta).map_err(|e| e.to_string())?;
                KvStoreResponse::AtomicIncrement { value }
            }
            Ok(KvStoreRequest::AtomicSwap { bucket, key, value }) => {
                atomic_swap(&bucket, &key, &value).map_err(|e| e.to_string())?;
                KvStoreResponse::AtomicSwap
            }
            Ok(KvStoreRequest::AtomicRead { bucket, key }) => {
                let value = atomic_read(&bucket, &key).map_err(|e| e.to_string())?;
                KvStoreResponse::AtomicRead { value }
            }
            Ok(KvStoreRequest::BatchRead { bucket, keys }) => {
                let values = batch_read(&bucket, &keys).map_err(|e| e.to_string())?;
                KvStoreResponse::BatchRead { values }
            }
            Ok(KvStoreRequest::BatchWrite { bucket, values }) => {
                batch_write(&bucket, values).map_err(|e| e.to_string())?;
                KvStoreResponse::BatchWrite
            }
            Ok(KvStoreRequest::BatchDelete { bucket, keys }) => {
                batch_delete(&bucket, &keys).map_err(|e| e.to_string())?;
                KvStoreResponse::BatchDelete
            }
            Ok(KvStoreRequest::ListKeys { bucket, cursor }) => {
                let KeyResponse { keys, cursor } =
                    list_keys(&bucket, cursor.as_deref()).map_err(|e| e.to_string())?;
                KvStoreResponse::ListKeys { keys, cursor }
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

fn open_bucket(id: &str) -> KvStoreResult<store::Bucket> {
    store::open(id).map_err(|e| KvStoreError::BucketOpen {
        id: id.to_string(),
        reason: e.to_string(),
    })
}

fn open_cas(id: &str, key: &str) -> KvStoreResult<atomics::Cas> {
    let bucket = open_bucket(id)?;
    atomics::Cas::new(&bucket, key).map_err(|e| KvStoreError::AtomicCasResource {
        bucket: id.to_string(),
        key: key.to_string(),
        reason: e.to_string(),
    })
}

fn read_value(bucket_id: &str, key: &str) -> KvStoreResult<Vec<u8>> {
    let bucket = open_bucket(bucket_id)?;
    bucket
        .get(key)
        .map_err(|e| KvStoreError::ReadKey {
            bucket: bucket_id.to_string(),
            key: key.to_string(),
            reason: e.to_string(),
        })?
        .ok_or_else(|| KvStoreError::MissingKey {
            bucket: bucket_id.to_string(),
            key: key.to_string(),
        })
}

fn write_value(bucket_id: &str, key: &str, value: &[u8]) -> KvStoreResult<()> {
    let bucket = open_bucket(bucket_id)?;
    bucket.set(key, value).map_err(|e| KvStoreError::WriteKey {
        bucket: bucket_id.to_string(),
        key: key.to_string(),
        reason: e.to_string(),
    })
}

fn atomic_increment(bucket_id: &str, key: &str, delta: i64) -> KvStoreResult<i64> {
    let bucket = open_bucket(bucket_id)?;
    atomics::increment(&bucket, key, delta).map_err(|e| KvStoreError::AtomicIncrement {
        bucket: bucket_id.to_string(),
        key: key.to_string(),
        delta,
        reason: e.to_string(),
    })
}

fn atomic_swap(bucket_id: &str, key: &str, value: &[u8]) -> KvStoreResult<()> {
    let cas = open_cas(bucket_id, key)?;
    atomics::swap(cas, value).map_err(|e| KvStoreError::AtomicSwap {
        bucket: bucket_id.to_string(),
        key: key.to_string(),
        reason: e.to_string(),
    })
}

fn atomic_read(bucket_id: &str, key: &str) -> KvStoreResult<Vec<u8>> {
    let cas = open_cas(bucket_id, key)?;
    cas.current()
        .map_err(|e| KvStoreError::AtomicRead {
            bucket: bucket_id.to_string(),
            key: key.to_string(),
            reason: e.to_string(),
        })?
        .ok_or_else(|| KvStoreError::MissingKey {
            bucket: bucket_id.to_string(),
            key: key.to_string(),
        })
}

fn batch_read(bucket_id: &str, keys: &[String]) -> KvStoreResult<HashMap<String, Vec<u8>>> {
    let bucket = open_bucket(bucket_id)?;
    Ok(batch::get_many(&bucket, keys)
        .map_err(|e| KvStoreError::BatchRead {
            bucket: bucket_id.to_string(),
            reason: e.to_string(),
        })?
        .into_iter()
        .flatten()
        .collect::<HashMap<_, _>>())
}

fn batch_write(bucket_id: &str, values: HashMap<String, Vec<u8>>) -> KvStoreResult<()> {
    let bucket = open_bucket(bucket_id)?;
    let values = values.into_iter().collect::<Vec<(String, Vec<u8>)>>();

    batch::set_many(&bucket, &values).map_err(|e| KvStoreError::BatchWrite {
        bucket: bucket_id.to_string(),
        reason: e.to_string(),
    })
}

fn batch_delete(bucket_id: &str, keys: &[String]) -> KvStoreResult<()> {
    let bucket = open_bucket(bucket_id)?;
    batch::delete_many(&bucket, keys).map_err(|e| KvStoreError::BatchDelete {
        bucket: bucket_id.to_string(),
        reason: e.to_string(),
    })
}

fn list_keys(bucket_id: &str, cursor: Option<&str>) -> KvStoreResult<store::KeyResponse> {
    let bucket = open_bucket(bucket_id)?;

    bucket
        .list_keys(cursor)
        .map_err(|e| KvStoreError::ListKeys {
            bucket: bucket_id.to_string(),
            reason: e.to_string(),
            cursor: cursor.map(|c| c.to_string()),
        })
}

export_layer_trigger_world!(Component);
