use wasmtime::component::Resource;

use super::{
    bucket_keys::{Key, KeyValueBucket},
    context::KeyValueState,
};
use crate::bindings::operator::world::wasi::keyvalue::batch;
use utils::storage::db::handles;

pub type BatchResult<T> = std::result::Result<T, batch::Error>;

impl<'a> KeyValueState<'a> {
    fn get_keys_batch(
        &self,
        bucket: &Resource<KeyValueBucket>,
        keys: Vec<String>,
    ) -> BatchResult<Vec<Key>> {
        self.get_keys(bucket, keys).map_err(batch::Error::Other)
    }
}

impl batch::Host for KeyValueState<'_> {
    fn get_many(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        original_keys: Vec<String>,
    ) -> BatchResult<Vec<Option<(String, Vec<u8>)>>> {
        let keys = self.get_keys_batch(&bucket, original_keys.clone())?;
        let mut results = Vec::with_capacity(keys.len());

        for (i, original_key) in original_keys.into_iter().enumerate() {
            let key = keys[i].to_string();
            match self.db.get(handles::KV_STORE, key) {
                Ok(Some(value)) => results.push(Some((original_key, value))),
                Ok(None) => results.push(None),
                Err(e) => {
                    return Err(batch::Error::Other(format!(
                        "Failed to read keyvalue store: {}",
                        e
                    )))
                }
            }
        }

        Ok(results)
    }

    fn set_many(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key_values: Vec<(String, Vec<u8>)>,
    ) -> BatchResult<()> {
        let prefix = self.get_key_prefix(&bucket).map_err(batch::Error::Other)?;

        for (key, value) in key_values {
            let key = Key::new(prefix.clone(), key).to_string();
            self.db
                .set(handles::KV_STORE, key, &value)
                .map_err(|e| batch::Error::Other(format!("Failed to set key: {}", e)))?;
        }

        Ok(())
    }

    fn delete_many(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        keys: Vec<String>,
    ) -> BatchResult<()> {
        let keys = self.get_keys_batch(&bucket, keys)?;

        for key in keys {
            let key = key.to_string();
            self.db
                .remove(handles::KV_STORE, key)
                .map(|_| ())
                .map_err(|e| batch::Error::Other(format!("Failed to delete key: {}", e)))?;
        }

        Ok(())
    }
}
