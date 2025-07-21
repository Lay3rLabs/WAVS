use wasmtime::component::Resource;

use super::context::KeyValueState;

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

    pub fn get_keys(
        &self,
        bucket: &Resource<KeyValueBucket>,
        keys: Vec<String>,
    ) -> std::result::Result<Vec<Key>, String> {
        let prefix = self.get_key_prefix(bucket)?;
        Ok(keys
            .into_iter()
            .map(|key| Key {
                prefix: prefix.clone(),
                key,
            })
            .collect())
    }
}

#[derive(Clone)]
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

impl Key {
    pub fn new(prefix: KeyPrefix, key: String) -> Self {
        Key { prefix, key }
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.prefix, self.key)
    }
}

pub struct KeyValueBucket {
    pub id: String,
}
