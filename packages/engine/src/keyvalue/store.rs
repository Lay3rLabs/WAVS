use utils::storage::db::Table;
use wasmtime::component::Resource;

use crate::bucket_keys::{Key, KeyPrefix, KeyValueBucket};
use crate::context::KeyValueState;
use crate::worker::bindings::world::wasi::keyvalue::store::{self, KeyResponse};
use redb::ReadableTable;

pub type StoreResult<T> = std::result::Result<T, store::Error>;

pub const KV_STORE_TABLE: Table<&str, Vec<u8>> = Table::new("kv_store");

impl<'a> KeyValueState<'a> {
    fn get_key_prefix_store(&self, bucket: &Resource<KeyValueBucket>) -> StoreResult<KeyPrefix> {
        self.get_key_prefix(bucket).map_err(store::Error::Other)
    }

    fn get_key_store(&self, bucket: &Resource<KeyValueBucket>, key_id: String) -> StoreResult<Key> {
        self.get_key(bucket, key_id).map_err(store::Error::Other)
    }

    pub fn set_store_value(&self, key: &Key, value: Vec<u8>) -> StoreResult<()> {
        self.db
            .set(KV_STORE_TABLE, key.to_string().as_ref(), &value)
            .map_err(|e| store::Error::Other(format!("Failed to set key in keyvalue store: {}", e)))
    }

    pub fn get_store_value(&self, key: &Key) -> StoreResult<Option<Vec<u8>>> {
        match self.db.get(KV_STORE_TABLE, key.to_string().as_ref()) {
            Ok(Some(kv)) => Ok(Some(kv.value())),
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
            .remove(KV_STORE_TABLE, key.to_string().as_ref())
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
            .map_table_read(KV_STORE_TABLE, |table| match table {
                Some(table) => {
                    let prefix_str = format!("{prefix}/");
                    let iter = match cursor {
                        Some(cursor) => {
                            let cursor_key = Key::new(prefix, cursor).to_string();
                            Box::new(table.range(cursor_key.as_str()..)?)
                        }
                        None => Box::new(table.iter()?),
                    }
                    .map(|i| i.map(|(key, _)| key));

                    let mut keys: Vec<String> = Vec::new();
                    let mut cursor = None;
                    let mut count = 0;
                    for key in iter {
                        let key = key?.value().to_string();
                        if key.starts_with(&prefix_str) {
                            count += 1;
                            if let Some(page_size) = self.page_size {
                                if count > page_size {
                                    cursor = Some(key);
                                    break;
                                }
                            }

                            let chopped_key = key[prefix_str.len()..].to_string();
                            keys.push(chopped_key);
                        } else {
                            break;
                        }
                    }

                    Ok(KeyResponse { keys, cursor })
                }
                None => Ok(KeyResponse {
                    keys: Vec::new(),
                    cursor: None,
                }),
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
