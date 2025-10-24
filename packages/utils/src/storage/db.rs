use std::sync::Arc;

use redb::{
    backends::InMemoryBackend, AccessGuard, Database, Key, ReadOnlyTable, ReadableDatabase,
    TableError, TypeName, Value,
};
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
    #[instrument(fields(subsys = "DbStorage"))]
    #[allow(clippy::result_large_err)]
    pub fn new() -> Result<Self, DBError> {
        let inner = Arc::new(Database::builder().create_with_backend(InMemoryBackend::new())?);

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

#[cfg(test)]
mod tests {

    use futures::stream::FuturesUnordered;
    use futures::StreamExt;
    use redb::backends::InMemoryBackend;
    use redb::{Database, TableDefinition};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::thread::{self, sleep};
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::task::spawn_blocking;

    use crate::storage::db::RedbStorage;

    const TABLE: TableDefinition<'static, u32, u32> = TableDefinition::new("TABLE");

    #[derive(Clone)]
    struct Counter {
        counts: Arc<std::sync::Mutex<HashMap<usize, usize>>>,
    }

    impl Counter {
        pub fn new() -> Self {
            Self {
                counts: Arc::new(std::sync::Mutex::new(HashMap::new())),
            }
        }

        pub fn increment(&self, thread_num: usize) {
            *self.counts.lock().unwrap().entry(thread_num).or_insert(0) += 1;
        }

        // only true if all threads have reached op_count
        // and there are thread_count threads
        pub fn reached(&self, thread_count: usize, op_count: usize) -> bool {
            let counts = self.counts.lock().unwrap();
            if counts.len() < thread_count - 1 {
                return false;
            }
            for count in counts.values() {
                if *count < op_count {
                    return false;
                }
            }
            true
        }
    }

    impl std::fmt::Debug for Counter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let counts = &*self.counts.lock().unwrap();
            counts.fmt(f)
        }
    }

    #[test]
    #[ignore]
    fn storage_multithreaded_in_memory() {
        storage_multithreaded_inner(StorageKind::InMemory, 20, 1000);
    }

    #[test]
    #[ignore]
    fn storage_multithreaded_on_disk() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db.redb");

        // incresaing the number of ops *dramatically* increases test time
        storage_multithreaded_inner(StorageKind::OnDisk(db_path), 10, 100);

        // just make sure we didn't drop it
        let _temp_dir = temp_dir;
    }

    fn storage_multithreaded_inner(
        storage_kind: StorageKind,
        task_target: usize,
        op_target: usize,
    ) {
        let storage = new_storage(storage_kind);

        // stash something at the beginning so we're guaranteed to get a valid read
        storage.set(TABLE, 1u32, &1u32).unwrap();

        let counter = Counter::new();

        for thread_num in 0..task_target {
            thread::spawn({
                let storage = storage.clone();
                let counter = counter.clone();
                move || loop {
                    if thread_num % 2 == 0 {
                        storage.set(TABLE, 1u32, &1u32).unwrap();
                    } else {
                        let value = storage.get(TABLE, 1).unwrap().unwrap().value();
                        assert_eq!(value, 1u32);
                    }

                    counter.increment(thread_num);
                    // give it a little time to let other threads run
                    sleep(Duration::from_millis(1));
                }
            });
        }

        loop {
            if counter.reached(task_target, op_target) {
                break;
            }

            // give it a little time to let other threads run
            sleep(Duration::from_millis(1));
        }
    }

    #[tokio::test]
    #[ignore]
    async fn storage_concurrent_in_memory() {
        storage_concurrent_inner(StorageKind::InMemory, 20, 1000).await;
    }

    #[tokio::test]
    #[ignore]
    async fn storage_concurrent_on_disk() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db.redb");

        // incresaing the number of ops *dramatically* increases test time
        storage_concurrent_inner(StorageKind::OnDisk(db_path), 10, 100).await;

        // just make sure we didn't drop it
        let _temp_dir = temp_dir;
    }

    async fn storage_concurrent_inner(
        storage_kind: StorageKind,
        task_target: usize,
        op_target: usize,
    ) {
        let storage = new_storage(storage_kind);

        // stash something at the beginning so we're guaranteed to get a valid read
        storage.set(TABLE, 1u32, &1u32).unwrap();

        let counter = Counter::new();

        let mut futures = FuturesUnordered::new();

        for task_num in 0..task_target {
            for _ in 0..op_target {
                futures.push({
                    let storage = storage.clone();
                    let counter = counter.clone();
                    async move {
                        if task_num % 2 == 0 {
                            spawn_blocking(move || {
                                storage.set(TABLE, 1u32, &1u32).unwrap();
                            })
                            .await
                            .unwrap();
                        } else {
                            let value = spawn_blocking(move || {
                                storage.get(TABLE, 1).unwrap().unwrap().value()
                            })
                            .await
                            .unwrap();

                            assert_eq!(value, 1u32);
                        }
                        counter.increment(task_num);
                    }
                });
            }
        }

        while (futures.next().await).is_some() {}

        if !counter.reached(task_target, op_target) {
            panic!("did not reach expected count")
        }
    }

    #[test]
    #[ignore]
    fn storage_serial_in_memory() {
        storage_serial_inner(StorageKind::InMemory, 20, 1000);
    }

    #[test]
    #[ignore]
    fn storage_serial_on_disk() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db.redb");

        // incresaing the number of ops *dramatically* increases test time
        storage_serial_inner(StorageKind::OnDisk(db_path), 10, 100);

        // just make sure we didn't drop it
        let _temp_dir = temp_dir;
    }

    fn storage_serial_inner(storage_kind: StorageKind, task_target: usize, op_target: usize) {
        let storage = new_storage(storage_kind);

        // stash something at the beginning so we're guaranteed to get a valid read
        storage.set(TABLE, 1u32, &1u32).unwrap();

        // just to make sure we're a bit closer to the other tests
        let counter = Counter::new();

        for task_num in 0..task_target {
            for _ in 0..op_target {
                if task_num % 2 == 0 {
                    storage.set(TABLE, 1u32, &1u32).unwrap();
                } else {
                    let value = storage.get(TABLE, 1).unwrap().unwrap().value();
                    assert_eq!(value, 1u32);
                }
                counter.increment(task_num);
            }
        }

        if !counter.reached(task_target, op_target) {
            panic!("did not reach expected count")
        }
    }

    enum StorageKind {
        InMemory,
        OnDisk(std::path::PathBuf),
    }

    fn new_storage(kind: StorageKind) -> RedbStorage {
        RedbStorage {
            inner: Arc::new(match kind {
                StorageKind::InMemory => Database::builder()
                    .create_with_backend(InMemoryBackend::new())
                    .unwrap(),
                StorageKind::OnDisk(path) => Database::create(path).unwrap(),
            }),
        }
    }
}
