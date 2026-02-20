use wasmtime::component::Resource;

use super::{
    bucket_keys::{Key, KeyValueBucket},
    context::KeyValueState,
};
use crate::bindings::operator::world::wasi::keyvalue::batch;

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
            if let Some(value) = self.db.kv_store.get_cloned(&key) {
                results.push(Some((original_key, value)));
            } else {
                results.push(None);
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
                .kv_store
                .insert(key, value)
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
            self.db.kv_store.remove(&key);
        }

        Ok(())
    }
}
