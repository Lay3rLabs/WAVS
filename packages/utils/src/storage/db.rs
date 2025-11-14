use std::hash::Hash;
use std::path::Path;
use std::sync::Arc;

use dashmap::mapref::multiple::RefMulti;
use dashmap::DashMap;
use tracing::instrument;

use wavs_types::{QuorumQueue, QuorumQueueId, Service, ServiceId};

/// Main database struct with hardcoded tables for better type safety and performance
#[derive(Clone)]
pub struct WavsDb {
    pub services: WavsDbTable<ServiceId, Service>,
    pub services_by_hash: WavsDbTable<[u8; 32], Service>,
    pub aggregator_services: WavsDbTable<ServiceId, ()>,
    pub quorum_queues: WavsDbTable<QuorumQueueId, QuorumQueue>,
    pub kv_store: WavsDbTable<String, Vec<u8>>,
    pub kv_atomics_counter: WavsDbTable<String, i64>,
}

impl WavsDb {
    /// Create a new database with all tables initialized
    #[instrument(fields(subsys = "WavsDb"))]
    pub fn new() -> Result<Self, DBError> {
        Ok(Self {
            services: WavsDbTable::new(None::<&str>)?,
            services_by_hash: WavsDbTable::new(None::<&str>)?,
            aggregator_services: WavsDbTable::new(None::<&str>)?,
            quorum_queues: WavsDbTable::new(None::<&str>)?,
            kv_store: WavsDbTable::new(None::<&str>)?,
            kv_atomics_counter: WavsDbTable::new(None::<&str>)?,
        })
    }
}

/// A table abstraction that hides the underlying DashMap implementation
/// and provides a clean API for database operations.
#[derive(Clone)]
pub struct WavsDbTable<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    inner: Arc<DashMap<K, V>>,
    filepath: Option<std::path::PathBuf>,
}

impl<K, V> Default for WavsDbTable<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            filepath: None,
        }
    }
}

impl<K, V> WavsDbTable<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Create a new table. In the future, this will open/load from a file.
    pub fn new(filepath: Option<impl AsRef<Path>>) -> Result<Self, DBError> {
        // TODO LATER: Open a file, load data, keep file handle for writing
        let filepath = filepath.map(|p| p.as_ref().to_path_buf());
        Ok(Self {
            inner: Arc::new(DashMap::new()),
            filepath,
        })
    }

    /// Get the filepath for this table (if any)
    pub fn filepath(&self) -> Option<&std::path::Path> {
        self.filepath.as_deref()
    }

    /// Get a cloned value from the table
    pub fn get_cloned(&self, key: &K) -> Option<V> {
        self.inner.get(key).map(|v| v.clone())
    }

    /// Work with a reference without exposing DashMap-specific types
    pub fn map_ref<T, F>(&self, key: &K, f: F) -> Option<T>
    where
        F: FnOnce(&V) -> T,
    {
        self.inner.get(key).map(|v| f(&v))
    }

    /// Insert a value into the table
    pub fn insert(&self, key: K, value: V) -> Result<(), DBError> {
        // TODO LATER: Write data to disk, e.g. in a separate thread
        self.inner.insert(key, value);
        Ok(())
    }

    /// Remove a value from the table
    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.remove(key).map(|(_, v)| v)
    }

    /// Check if a key exists in the table
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Clear all entries from the table
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Iterate over all entries in the table
    pub fn iter(&self) -> WavsDbIter<'_, K, V> {
        WavsDbIter {
            inner: self.inner.iter(),
        }
    }

    /// Execute a function with read access to the table
    pub fn with_read<F, R>(&self, f: F) -> Result<R, DBError>
    where
        F: FnOnce(&WavsDbTable<K, V>) -> Result<R, DBError>,
    {
        f(self)
    }
}

/// Iterator for WavsDbTable that hides DashMap-specific types
pub struct WavsDbIter<'a, K, V> {
    inner: dashmap::iter::Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for WavsDbIter<'a, K, V>
where
    K: Eq + Hash,
{
    type Item = WavsDbEntry<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(WavsDbEntry)
    }
}

/// Entry for WavsDbTable that hides DashMap-specific types
pub struct WavsDbEntry<'a, K, V>(RefMulti<'a, K, V>);

impl<'a, K, V> WavsDbEntry<'a, K, V>
where
    K: Eq + Hash,
{
    pub fn pair(&self) -> (&K, &V) {
        (self.0.key(), self.0.value())
    }
}

pub type DBError = anyhow::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn wavsdb_table_basic_operations() {
        let table: WavsDbTable<String, TestStruct> = WavsDbTable::new(None::<&str>).unwrap();
        let key = "test_key".to_string();
        let value = TestStruct {
            name: "demo".to_string(),
            value: 99,
        };

        // Test get_cloned on empty table
        assert!(table.get_cloned(&key).is_none());

        // Test insert and get_cloned
        table.insert(key.clone(), value.clone()).unwrap();
        let retrieved = table.get_cloned(&key);
        assert_eq!(retrieved, Some(value.clone()));

        // Test contains_key
        assert!(table.contains_key(&key));
        assert!(!table.contains_key(&"nonexistent".to_string()));

        // Test remove
        let removed = table.remove(&key);
        assert_eq!(removed, Some(value));
        assert!(!table.contains_key(&key));
    }

    #[test]
    fn wavsdb_table_map_ref() {
        let table: WavsDbTable<String, i32> = WavsDbTable::new(None::<&str>).unwrap();
        let key = "number".to_string();
        table.insert(key.clone(), 42).unwrap();

        // Test map_ref to transform value without cloning
        let doubled = table.map_ref(&key, |v| v * 2);
        assert_eq!(doubled, Some(84));

        // Test map_ref on nonexistent key
        let none_result = table.map_ref(&"nonexistent".to_string(), |v| v * 2);
        assert_eq!(none_result, None);
    }

    #[test]
    fn wavsdb_table_iteration() {
        let table: WavsDbTable<String, TestStruct> = WavsDbTable::new(None::<&str>).unwrap();

        // Insert test data
        table
            .insert(
                "alpha".to_string(),
                TestStruct {
                    name: "a".to_string(),
                    value: 1,
                },
            )
            .unwrap();

        table
            .insert(
                "beta".to_string(),
                TestStruct {
                    name: "b".to_string(),
                    value: 2,
                },
            )
            .unwrap();

        // Collect all entries
        let mut collected: Vec<(String, i32)> = table
            .iter()
            .map(|entry| {
                let (key, value) = entry.pair();
                (key.clone(), value.value)
            })
            .collect();

        // Sort for consistent ordering (iteration order is not guaranteed)
        collected.sort();
        assert_eq!(collected, vec![("alpha".into(), 1), ("beta".into(), 2)]);
    }

    #[test]
    fn wavsdb_basic_operations() {
        let db = WavsDb::new().unwrap();

        // Test basic operations with a simple test struct instead of Service
        use wavs_types::ServiceId;
        let service_id = ServiceId::hash(b"test-service");
        let service = Service {
            name: "test-service".to_string(),
            workflows: std::collections::BTreeMap::new(),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain: "evm:anvil".parse().unwrap(),
                address: alloy_primitives::Address::ZERO,
            },
        };

        assert!(db.services.get_cloned(&service_id).is_none());
        db.services
            .insert(service_id.clone(), service.clone())
            .unwrap();

        let retrieved = db.services.get_cloned(&service_id);
        assert_eq!(retrieved, Some(service.clone()));

        assert!(db.services.contains_key(&service_id));

        let removed = db.services.remove(&service_id);
        assert_eq!(removed, Some(service));
        assert!(!db.services.contains_key(&service_id));
    }

    #[test]
    fn wavsdb_kv_operations() {
        let db = WavsDb::new().unwrap();

        let key = "test_key".to_string();
        let value = b"test_value".to_vec();

        // Test KV operations
        assert!(db.kv_store.get_cloned(&key).is_none());
        db.kv_store.insert(key.clone(), value.clone()).unwrap();

        let retrieved = db.kv_store.get_cloned(&key);
        assert_eq!(retrieved, Some(value.clone()));

        assert!(db.kv_store.contains_key(&key));

        let removed = db.kv_store.remove(&key);
        assert_eq!(removed, Some(b"test_value".to_vec()));
        assert!(!db.kv_store.contains_key(&key));
    }

    #[test]
    fn wavsdb_counter_operations() {
        let db = WavsDb::new().unwrap();

        let key = "counter".to_string();
        let value = 42i64;

        // Test counter operations
        assert!(db.kv_atomics_counter.get_cloned(&key).is_none());
        db.kv_atomics_counter.insert(key.clone(), value).unwrap();

        let retrieved = db.kv_atomics_counter.get_cloned(&key);
        assert_eq!(retrieved, Some(value));

        assert!(db.kv_atomics_counter.contains_key(&key));

        let removed = db.kv_atomics_counter.remove(&key);
        assert_eq!(removed, Some(value));
        assert!(!db.kv_atomics_counter.contains_key(&key));
    }

    #[test]
    fn table_clear() {
        let table: WavsDbTable<String, i32> = WavsDbTable::new(None::<&str>).unwrap();

        // Insert some data
        table.insert("a".to_string(), 1).unwrap();
        table.insert("b".to_string(), 2).unwrap();

        assert!(table.contains_key(&"a".to_string()));
        assert!(table.contains_key(&"b".to_string()));

        // Clear the table
        table.clear();

        assert!(!table.contains_key(&"a".to_string()));
        assert!(!table.contains_key(&"b".to_string()));
    }

    #[test]
    fn table_with_read() {
        let table: WavsDbTable<String, i32> = WavsDbTable::new(None::<&str>).unwrap();

        // Insert test data
        table.insert("x".to_string(), 10).unwrap();
        table.insert("y".to_string(), 20).unwrap();

        // Use with_read to count entries
        let count = table.with_read(|t| Ok(t.iter().count())).unwrap();

        assert_eq!(count, 2);
    }
}
