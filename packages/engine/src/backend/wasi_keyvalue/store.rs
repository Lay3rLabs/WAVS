use utils::storage::db::handles;
use wasmtime::component::Resource;

use super::bucket_keys::{Key, KeyPrefix, KeyValueBucket};
use super::context::KeyValueState;
use crate::bindings::operator::world::wasi::keyvalue::store::{self, KeyResponse};
use serde_json;

pub type StoreResult<T> = std::result::Result<T, store::Error>;

impl<'a> KeyValueState<'a> {
    fn get_key_prefix_store(&self, bucket: &Resource<KeyValueBucket>) -> StoreResult<KeyPrefix> {
        self.get_key_prefix(bucket).map_err(store::Error::Other)
    }

    fn get_key_store(&self, bucket: &Resource<KeyValueBucket>, key_id: String) -> StoreResult<Key> {
        self.get_key(bucket, key_id).map_err(store::Error::Other)
    }

    pub fn set_store_value(&self, key: &Key, value: Vec<u8>) -> StoreResult<()> {
        self.db
            .set(handles::KV_STORE, key.to_string(), &value)
            .map_err(|e| store::Error::Other(format!("Failed to set key in keyvalue store: {}", e)))
    }

    pub fn get_store_value(&self, key: &Key) -> StoreResult<Option<Vec<u8>>> {
        match self.db.get(handles::KV_STORE, key.to_string()) {
            Ok(Some(kv)) => Ok(Some(kv)),
            Ok(None) => Ok(None),
            Err(err) => Err(store::Error::Other(format!(
                "Failed to get key from keyvalue store: {}",
                err
            ))),
        }
    }
}

impl store::Host for KeyValueState<'_> {
    fn open(&mut self, id: String) -> StoreResult<Resource<KeyValueBucket>> {
        self.resource_table
            .push(KeyValueBucket { id })
            .map_err(|e| store::Error::Other(format!("Failed to open keyvalue bucket: {}", e)))
    }
}

impl store::HostBucket for KeyValueState<'_> {
    fn get(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key_id: String,
    ) -> StoreResult<Option<Vec<u8>>> {
        let key = self.get_key_store(&bucket, key_id)?;
        self.get_store_value(&key)
    }

    fn set(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key_id: String,
        value: Vec<u8>,
    ) -> StoreResult<()> {
        let key = self.get_key_store(&bucket, key_id)?;
        self.set_store_value(&key, value)
    }

    fn delete(&mut self, bucket: Resource<KeyValueBucket>, key: String) -> StoreResult<()> {
        let key = self.get_key_store(&bucket, key)?;
        self.db
            .remove(handles::KV_STORE, key.to_string())
            .map(|_| ())
            .map_err(|e| {
                store::Error::Other(format!("Failed to delete key from keyvalue store: {}", e))
            })
    }

    fn exists(&mut self, bucket: Resource<KeyValueBucket>, key: String) -> StoreResult<bool> {
        self.get(bucket, key).map(|x| x.is_some())
    }

    // TODO - test me!
    // https://github.com/Lay3rLabs/WAVS/issues/767
    fn list_keys(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        cursor: Option<String>,
    ) -> StoreResult<KeyResponse> {
        let prefix = self.get_key_prefix_store(&bucket)?;
        let res = self
            .db
            .with_table_read(handles::KV_STORE, |table| {
                let prefix_str = format!("{prefix}/");

                let mut all_keys: Vec<String> = Vec::new();

                // Collect all keys that match the prefix
                for entry in table.iter() {
                    let (key_bytes, _value_bytes) = entry.pair();
                    if let Ok(key_string) = serde_json::from_slice::<String>(key_bytes) {
                        if key_string.starts_with(&prefix_str) {
                            all_keys.push(key_string);
                        }
                    }
                }

                // Sort keys for consistent iteration
                all_keys.sort();

                // Apply cursor if provided
                let start_idx = if let Some(ref cursor_str) = cursor {
                    let cursor_key = Key::new(prefix, cursor_str.clone()).to_string();
                    all_keys
                        .iter()
                        .position(|k| *k >= cursor_key)
                        .unwrap_or(all_keys.len())
                } else {
                    0
                };

                let keys_from_cursor = &all_keys[start_idx..];

                let mut keys: Vec<String> = Vec::new();
                let mut next_cursor = None;
                let mut count = 0;

                for key in keys_from_cursor {
                    if key.starts_with(&prefix_str) {
                        count += 1;
                        if let Some(page_size) = self.page_size {
                            if count > page_size {
                                next_cursor = Some(key.clone());
                                break;
                            }
                        }

                        let chopped_key = key[prefix_str.len()..].to_string();
                        keys.push(chopped_key);
                    } else {
                        break;
                    }
                }

                Ok(KeyResponse {
                    keys,
                    cursor: next_cursor,
                })
            })
            .map_err(|e| {
                store::Error::Other(format!("Failed to list keys in keyvalue store: {}", e))
            })?;

        Ok(res)
    }

    fn drop(
        &mut self,
        bucket: Resource<KeyValueBucket>,
    ) -> std::result::Result<(), wasmtime::Error> {
        self.resource_table.delete(bucket)?;
        Ok(())
    }
}
