use wasmtime::component::Resource;

use crate::{
    bindings::world::wasi::keyvalue::batch,
    bucket_keys::{Key, KeyValueBucket},
    context::KeyValueState,
    store::KV_STORE_TABLE,
};

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
        self.db
            .map_table_read(KV_STORE_TABLE, |table| match table {
                Some(table) => {
                    let mut results = Vec::with_capacity(keys.len());
                    for (i, original_key) in original_keys.into_iter().enumerate() {
                        let key = keys[i].to_string();
                        results.push(table.get(&*key)?.map(|value| (original_key, value.value())));
                    }
                    Ok(results)
                }
                None => Ok(Vec::new()),
            })
            .map_err(|e| batch::Error::Other(format!("Failed to read keyvalue store: {}", e)))
    }

    fn set_many(
        &mut self,
        bucket: Resource<batch::Bucket>,
        key_values: Vec<(String, Vec<u8>)>,
    ) -> BatchResult<()> {
        // TODO - try to make db.map_table_write()
        let prefix = self.get_key_prefix(&bucket).map_err(batch::Error::Other)?;
        for (key, value) in key_values {
            let key = Key::new(prefix.clone(), key);

            self.db
                .set(KV_STORE_TABLE, key.to_string().as_ref(), &value)
                .map_err(|e| {
                    batch::Error::Other(format!("Failed to set key in keyvalue batch: {}", e))
                })?;
        }

        Ok(())
    }

    fn delete_many(
        &mut self,
        bucket: Resource<batch::Bucket>,
        keys: Vec<String>,
    ) -> BatchResult<()> {
        // TODO - try to make db.map_table_write()
        let keys = self.get_keys_batch(&bucket, keys)?;
        for key in keys {
            self.db
                .remove(KV_STORE_TABLE, key.to_string().as_ref())
                .map_err(|e| {
                    batch::Error::Other(format!("Failed to delete key in keyvalue batch: {}", e))
                })?;
        }

        Ok(())
    }
}
