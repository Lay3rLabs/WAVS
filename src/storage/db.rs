use std::path::Path;

use redb::{AccessGuard, Database, TableError};

use super::prelude::*;

pub struct RedbStorage {
    db: Database,
}

impl RedbStorage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, KVStorageError> {
        let db = redb::Database::create(path)?;
        Ok(RedbStorage { db })
    }
}

impl KVStorage for RedbStorage {
    fn set<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
        value: &V::SelfType<'_>,
    ) -> Result<(), KVStorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn get<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<Option<AccessGuard<'static, V>>, KVStorageError> {
        let read_txn = self.db.begin_read()?;
        match read_txn.open_table(table) {
            Ok(table) => Ok(table.get(key)?),
            // If we read before the first write, we get this error.
            // Just act like get returned None (cuz key surely doesn't exist)
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn remove<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<(), KVStorageError> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.remove(key)?;
        }
        write_txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
