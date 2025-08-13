use utils::storage::db::Table;
use wasmtime::component::Resource;

use super::{
    bucket_keys::{Key, KeyValueBucket},
    context::KeyValueState,
};
use crate::bindings::operator::world::wasi::keyvalue::atomics;

pub type AtomicsResult<T> = std::result::Result<T, atomics::Error>;
pub type CasResult<T> = std::result::Result<T, atomics::CasError>;

const KV_ATOMICS_TABLE_COUNTER: Table<&str, i64> = Table::new("kv_atomics_counter");

impl<'a> KeyValueState<'a> {
    fn get_cas_atomics(&self, cas: &Resource<KeyValueCas>) -> AtomicsResult<&KeyValueCas> {
        self.resource_table
            .get::<KeyValueCas>(cas)
            .map_err(|e| atomics::Error::Other(format!("Failed to get keyvalue cas: {}", e)))
    }

    fn get_key_atomics(
        &self,
        bucket: &Resource<KeyValueBucket>,
        key: String,
    ) -> AtomicsResult<Key> {
        self.get_key(bucket, key).map_err(atomics::Error::Other)
    }

    fn get_atomic_count(&mut self, key: &Key) -> AtomicsResult<Option<i64>> {
        match self
            .db
            .get(KV_ATOMICS_TABLE_COUNTER, key.to_string().as_ref())
        {
            Ok(Some(kv)) => Ok(Some(kv.value())),
            Ok(None) => Ok(None),
            Err(err) => Err(atomics::Error::Other(format!(
                "Failed to get key from keyvalue atomics: {}",
                err
            ))),
        }
    }

    fn save_atomic_count(&mut self, key: &Key, value: i64) -> AtomicsResult<()> {
        self.db
            .set(KV_ATOMICS_TABLE_COUNTER, key.to_string().as_ref(), &value)
            .map_err(|e| {
                atomics::Error::Other(format!("Failed to set key in keyvalue atomics: {}", e))
            })
    }
}

impl atomics::Host for KeyValueState<'_> {
    fn increment(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key_id: String,
        delta: i64,
    ) -> AtomicsResult<i64> {
        let key = self.get_key_atomics(&bucket, key_id)?;

        let mut count = self.get_atomic_count(&key)?.unwrap_or(0);
        count += delta;
        self.save_atomic_count(&key, count)?;
        Ok(count)
    }

    fn swap(&mut self, cas: Resource<KeyValueCas>, value: Vec<u8>) -> CasResult<()> {
        let cas = self
            .get_cas_atomics(&cas)
            .map_err(atomics::CasError::StoreError)?;
        self.set_store_value(&cas.key, value)
            .map_err(atomics::CasError::StoreError)
    }
}

impl atomics::HostCas for KeyValueState<'_> {
    fn new(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key_id: String,
    ) -> AtomicsResult<Resource<KeyValueCas>> {
        let key = self.get_key_atomics(&bucket, key_id)?;
        let cas = KeyValueCas { key };
        self.resource_table
            .push(cas)
            .map_err(|e| atomics::Error::Other(format!("Failed to create keyvalue cas: {}", e)))
    }

    fn current(&mut self, cas: Resource<KeyValueCas>) -> AtomicsResult<Option<Vec<u8>>> {
        let cas = self.get_cas_atomics(&cas)?;
        self.get_store_value(&cas.key)
            .map_err(|e| atomics::Error::Other(e.to_string()))
    }

    fn drop(&mut self, cas: Resource<KeyValueCas>) -> std::result::Result<(), wasmtime::Error> {
        self.resource_table.delete(cas)?;
        Ok(())
    }
}

pub struct KeyValueCas {
    pub key: Key,
}
