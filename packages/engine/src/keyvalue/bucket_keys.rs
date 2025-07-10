use wasmtime::component::Resource;

use crate::context::KeyValueState;

impl<'a> KeyValueState<'a> {
    pub fn get_bucket(
        &self,
        bucket: &Resource<KeyValueBucket>,
    ) -> std::result::Result<&KeyValueBucket, String> {
        self.resource_table
            .get::<KeyValueBucket>(bucket)
            .map_err(|e| format!("Failed to get keyvalue bucket: {}", e))
    }

    pub fn get_key_prefix(
        &self,
        bucket: &Resource<KeyValueBucket>,
    ) -> std::result::Result<KeyPrefix, String> {
        Ok(KeyPrefix {
            namespace: self.namespace.clone(),
            bucket_id: self.get_bucket(bucket)?.id.clone(),
        })
    }

    pub fn get_key(
        &self,
        bucket: &Resource<KeyValueBucket>,
        key: String,
    ) -> std::result::Result<Key, String> {
        let prefix = self.get_key_prefix(bucket)?;
        Ok(Key {
            prefix,
            key: key.to_string(),
        })
    }
}

pub struct KeyPrefix {
    namespace: String,
    bucket_id: String,
}

impl std::fmt::Display for KeyPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.bucket_id)
    }
}

pub struct Key {
    prefix: KeyPrefix,
    key: String,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.prefix, self.key)
    }
}

pub struct KeyValueBucket {
    pub id: String,
}
