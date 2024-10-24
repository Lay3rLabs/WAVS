use std::path::Path;

use redb::{AccessGuard, Database};

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
        let table = read_txn.open_table(table)?;
        let value = table.get(key)?;
        Ok(value)
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

    const T1: Table<u32, String> = Table::new("t1");

    #[test]
    fn test_set_once_and_get() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let store = RedbStorage::new(file.path()).unwrap();

        // Note, currently need to set one value in the table to create it, before it can be queried
        // We should add some init functions for this
        store.set(T1, 0, &"".to_string()).unwrap();
        store.remove(T1, 0).unwrap();

        let empty = store.get(T1, 17).unwrap();
        assert!(empty.is_none());

        let data = "hello".to_string();
        store.set(T1, 17, &data).unwrap();
        let full = store.get(T1, 17).unwrap().unwrap();
        assert_eq!(data, full.value());
    }
}
