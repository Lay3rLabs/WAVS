use std::borrow::ToOwned;
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

// impl KVStorage for RedbStorage {
impl RedbStorage {
    fn set<K: Key, V: Value + 'static>(
        &mut self,
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

    fn get<'a, 'b, K: Key, V: Value + 'static>(
        &mut self,
        table: Table<K, V>,
        key: K::SelfType<'a>,
    ) -> Result<Option<AccessGuard<'static, V>>, KVStorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(table)?;
        let value = table.get(key)?;
        Ok(value)
    }
}
