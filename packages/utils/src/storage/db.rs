use std::{path::Path, sync::Arc};

use redb::{AccessGuard, Database, Key, ReadOnlyTable, TableError, TypeName, Value};
use serde::{de::Deserialize, Serialize};
use std::any::type_name;
use tracing::instrument;

#[derive(Clone)]
pub struct RedbStorage {
    pub inner: Arc<Database>,
}

pub type Table<K, V> = redb::TableDefinition<'static, K, V>;
pub type DBError = redb::Error;

impl RedbStorage {
    #[instrument(skip(path), fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn new(path: impl AsRef<Path>) -> Result<Self, DBError> {
        let path = if path.as_ref().is_dir() {
            tracing::warn!(
                "RedbStorage: Path {} is a directory, creating database in it.",
                path.as_ref().display()
            );

            path.as_ref().join("db").to_path_buf()
        } else {
            path.as_ref().to_path_buf()
        };
        let inner = Arc::new(redb::Database::create(path)?);
        Ok(RedbStorage { inner })
    }
}

impl RedbStorage {
    #[instrument(skip(self, table), fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn set<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
        value: &V::SelfType<'_>,
    ) -> Result<(), DBError> {
        let write_txn = self.inner.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    #[instrument(skip(self, table), fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn get<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<Option<AccessGuard<'static, V>>, DBError> {
        let read_txn = self.inner.begin_read()?;
        match read_txn.open_table(table) {
            Ok(table) => Ok(table.get(key)?),
            // If we read before the first write, we get this error.
            // Just act like get returned None (cuz key surely doesn't exist)
            Err(TableError::TableDoesNotExist(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    #[instrument(skip(self, table), fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn remove<K: Key, V: Value + 'static>(
        &self,
        table: Table<K, V>,
        key: K::SelfType<'_>,
    ) -> Result<(), DBError> {
        let write_txn = self.inner.begin_write()?;
        {
            let mut table = write_txn.open_table(table)?;
            table.remove(key)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    // TODO: this could just be an internal helper method for get(), range(), etc.
    #[instrument(skip(self, table, f), fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn map_table_read<'a, K, V, F, R>(&self, table: Table<K, V>, f: F) -> Result<R, DBError>
    where
        K: Key + 'a,
        V: Value + 'a,
        F: FnOnce(Option<ReadOnlyTable<K, V>>) -> Result<R, DBError>,
    {
        let read_txn = self.inner.begin_read()?;
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
    type SelfType<'a>
        = T
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
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
