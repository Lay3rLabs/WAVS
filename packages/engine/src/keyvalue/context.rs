use utils::storage::db::{RedbStorage, Table};
use wasmtime::component::{HasData, Resource};
use wasmtime_wasi::ResourceTable;

use crate::bindings::world::wasi::keyvalue::store::{self, KeyResponse};
use crate::{EngineError, HostComponent};

const KV_TABLE: Table<&str, Vec<u8>> = Table::new("kv_store");

type StoreResult<T> = std::result::Result<T, store::Error>;

#[derive(Clone)]
pub struct KeyValueCtx {
    db: RedbStorage,
    // should be a unique identifier for the keyvalue store, e.g. per-service
    // this is *not* the namespace per-bucket, each KeyValueCtx may have multiple buckets
    namespace: String,
    // for pagination
    page_size: Option<usize>,
}

impl KeyValueCtx {
    pub fn new(db: RedbStorage, namespace: String) -> Self {
        KeyValueCtx {
            db,
            namespace,
            page_size: None,
        }
    }
    pub fn add_to_linker(
        linker: &mut wasmtime::component::Linker<HostComponent>,
    ) -> Result<(), EngineError> {
        store::add_to_linker::<HostComponent, KeyValueCtx>(linker, |state| {
            KeyValueState::new(
                state.keyvalue_ctx.db.clone(),
                state.keyvalue_ctx.namespace.clone(),
                &mut state.table,
                state.keyvalue_ctx.page_size,
            )
        })
        .map_err(EngineError::AddToLinker)?;

        Ok(())
    }
}

impl HasData for KeyValueCtx {
    type Data<'a> = KeyValueState<'a>;
}

pub struct KeyValueState<'a> {
    db: RedbStorage,
    namespace: String,
    resource_table: &'a mut ResourceTable,
    page_size: Option<usize>,
}

impl<'a> KeyValueState<'a> {
    pub fn new(
        db: RedbStorage,
        namespace: String,
        resource_table: &'a mut ResourceTable,
        page_size: Option<usize>,
    ) -> Self {
        Self {
            db,
            namespace,
            resource_table,
            page_size,
        }
    }

    pub fn get_bucket(&self, bucket: Resource<KeyValueBucket>) -> StoreResult<&KeyValueBucket> {
        self.resource_table
            .get::<KeyValueBucket>(&bucket)
            .map_err(|e| store::Error::Other(format!("Failed to get keyvalue bucket: {}", e)))
    }

    pub fn key(&self, bucket: Resource<KeyValueBucket>, key: &str) -> StoreResult<String> {
        let prefix = self.prefix(bucket)?;
        Ok(format!("{prefix}/{key}"))
    }

    pub fn prefix(&self, bucket: Resource<KeyValueBucket>) -> StoreResult<String> {
        let bucket_id = &self.get_bucket(bucket)?.id;
        Ok(format!("{}/{}", self.namespace, bucket_id))
    }
}

pub struct KeyValueBucket {
    id: String,
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
        key: String,
    ) -> StoreResult<Option<Vec<u8>>> {
        let key = self.key(bucket, &key)?;

        match self.db.get(KV_TABLE, key.as_ref()) {
            Ok(Some(kv)) => Ok(Some(kv.value())),
            Ok(None) => Ok(None),
            Err(err) => Err(store::Error::Other(format!(
                "Failed to get key from keyvalue store: {}",
                err
            ))),
        }
    }

    fn set(
        &mut self,
        bucket: Resource<KeyValueBucket>,
        key: String,
        value: Vec<u8>,
    ) -> StoreResult<()> {
        let key = self.key(bucket, &key)?;

        self.db.set(KV_TABLE, key.as_ref(), &value).map_err(|e| {
            store::Error::Other(format!("Failed to set key in keyvalue store: {}", e))
        })?;

        Ok(())
    }

    fn delete(&mut self, bucket: Resource<KeyValueBucket>, key: String) -> StoreResult<()> {
        let key = self.key(bucket, &key)?;
        self.db.remove(KV_TABLE, key.as_ref()).map_err(|e| {
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
        let prefix = self.prefix(bucket)?;
        let cursor = cursor.unwrap_or(prefix.clone());

        let res = self
            .db
            .map_table_read(KV_TABLE, |table| match table {
                Some(table) => {
                    let cursor: &str = &cursor;
                    let iter = table.range(cursor..)?.map(|i| i.map(|(key, _)| key));

                    let mut keys: Vec<String> = Vec::new();
                    let mut cursor = None;
                    let mut count = 0;
                    for key in iter {
                        let key = key?.value().to_string();
                        if key.starts_with(&prefix) {
                            count += 1;
                            if let Some(page_size) = self.page_size {
                                if count >= page_size {
                                    cursor = Some(key);
                                    break;
                                }
                            }

                            keys.push(key[prefix.len()..].to_string());
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
