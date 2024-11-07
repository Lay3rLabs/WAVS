use std::path::Path;

use redb::{AccessGuard, Database, Key, ReadOnlyTable, TableError, TypeName, Value};
use serde::{de::Deserialize, Serialize};
use std::any::type_name;

pub struct RedbStorage {
    db: Database,
}

pub type Table<K, V> = redb::TableDefinition<'static, K, V>;
pub type DBError = redb::Error;

impl RedbStorage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, DBError> {
        let db = redb::Database::create(path)?;
        Ok(RedbStorage { db })
    }
}

impl RedbStorage {
    pub fn set<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
        value: &V::SelfType<'_>,
    ) -> Result<(), DBError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn get<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<Option<AccessGuard<'static, V>>, DBError> {
        let read_txn = self.db.begin_read()?;
        match read_txn.open_table(table) {
            Ok(table) => Ok(table.get(key)?),
            // If we read before the first write, we get this error.
            // Just act like get returned None (cuz key surely doesn't exist)
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn remove<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<(), DBError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.remove(key)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    // TODO: this could just be an internal helper method for get(), range(), etc.
    pub fn map_table_read<'a, K, V, F, R>(&self, table: Table<K, V>, f: F) -> Result<R, DBError>
    where
        K: Key + 'a,
        V: Value + 'a,
        F: FnOnce(Option<ReadOnlyTable<K, V>>) -> Result<R, DBError>,
    {
        let read_txn = self.db.begin_read()?;
        match read_txn.open_table(table) {
            Ok(table) => f(Some(table)),
            Err(TableError::TableDoesNotExist(_)) => f(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// Wrapper type to handle keys and values using bincode serialization
#[derive(Debug, Clone)]
pub struct JSON<T>(pub T);

impl<T> Value for JSON<T>
where
    T: std::fmt::Debug + Serialize + for<'a> Deserialize<'a>,
{
    type SelfType<'a> = T
    where
        Self: 'a;

    type AsBytes<'a> = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        serde_json::from_slice(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        serde_json::to_vec(value).unwrap()
    }

    fn type_name() -> TypeName {
        TypeName::new(&format!("JSON<{}>", type_name::<T>()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        apis::{
            dispatcher::{Component, Service, ServiceStatus, Workflow},
            Trigger, ID,
        },
        Digest,
    };

    use super::*;

    use redb::ReadableTable;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
    pub struct Demo {
        pub name: String,
        pub age: u16,
        pub nicknames: Vec<String>,
    }

    // basic types
    const T1: Table<u32, String> = Table::new("t1");

    // json types with &str key
    const TJ: Table<&str, JSON<Demo>> = Table::new("tj");

    #[test]
    fn test_set_once_and_get() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let store = RedbStorage::new(file.path()).unwrap();

        let empty = store.get(T1, 17).unwrap();
        assert!(empty.is_none());

        let data = "hello".to_string();
        store.set(T1, 17, &data).unwrap();
        let full = store.get(T1, 17).unwrap().unwrap();
        assert_eq!(data, full.value());
    }

    #[test]
    fn test_json_storage() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let store = RedbStorage::new(file.path()).unwrap();

        let empty = store.get(TJ, "john").unwrap();
        assert!(empty.is_none());

        let data = Demo {
            name: "John".to_string(),
            age: 28,
            nicknames: vec!["Johnny".to_string(), "Mr. Rocket".to_string()],
        };
        store.set(TJ, "john", &data).unwrap();
        let full = store.get(TJ, "john").unwrap().unwrap();
        assert_eq!(data, full.value());
    }

    #[test]
    fn db_service_store() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let storage = RedbStorage::new(file.path()).unwrap();

        const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("temp-services");

        let service_id = ID::new("service-id-1").unwrap();

        let components: BTreeMap<ID, Component> = [
            (
                ID::new("component-id-1").unwrap(),
                Component::new(&Digest::new(b"digest-1")),
            ),
            (
                ID::new("component-id-2").unwrap(),
                Component::new(&Digest::new(b"digest-2")),
            ),
        ]
        .into();

        let workflows: BTreeMap<ID, Workflow> = [
            (
                ID::new("workflow-id-1").unwrap(),
                Workflow {
                    trigger: Trigger::queue("fake-addr-1", 5),
                    component: ID::new("component-id-1").unwrap(),
                    submit: None,
                },
            ),
            (
                ID::new("workflow-id-2").unwrap(),
                Workflow {
                    trigger: Trigger::queue("fake-addr-2", 5),
                    component: ID::new("component-id-2").unwrap(),
                    submit: None,
                },
            ),
        ]
        .into();

        let service = Service {
            id: service_id.clone(),
            name: service_id.to_string(),
            components,
            workflows,
            status: ServiceStatus::Active,
            testable: true,
        };

        storage.set(SERVICE_TABLE, &service_id, &service).unwrap();

        let service_stored = storage.get(SERVICE_TABLE, &service_id).unwrap().unwrap();

        assert_eq!(service, service_stored.value());

        // can read keys via iterator
        let keys = storage
            .map_table_read(SERVICE_TABLE, |table| {
                Ok(table
                    .unwrap()
                    .iter()
                    .unwrap()
                    .map(|i| {
                        let (k, _) = i.unwrap();
                        k.value().to_string()
                    })
                    .map(|k| k)
                    .collect::<Vec<String>>())
            })
            .unwrap();

        assert_eq!(vec![service_id.to_string()], keys);

        let values = storage
            .map_table_read(SERVICE_TABLE, |table| {
                Ok(table
                    .unwrap()
                    .iter()
                    .unwrap()
                    .map(|i| {
                        let (_, v) = i.unwrap();
                        v.value()
                    })
                    .map(|k| k)
                    .collect::<Vec<Service>>())
            })
            .unwrap();

        assert_eq!(vec![service], values);
    }
}
