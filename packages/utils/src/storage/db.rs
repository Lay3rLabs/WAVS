use std::any::Any;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::anyhow;
use dashmap::mapref::entry::Entry;
use dashmap::mapref::multiple::RefMulti;
use dashmap::DashMap;
use tracing::instrument;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    Services,
    ServicesByHash,
    AggregatorServices,
    QuorumQueues,
    KvStore,
    KvAtomicsCounter,
    Test(&'static str),
}

impl Table {
    pub fn name(&self) -> &'static str {
        match self {
            Table::Services => "services",
            Table::ServicesByHash => "services-by-hash",
            Table::AggregatorServices => "aggregator-services",
            Table::QuorumQueues => "quorum_queues",
            Table::KvStore => "kv_store",
            Table::KvAtomicsCounter => "kv_atomics_counter",
            Table::Test(name) => name,
        }
    }
}

#[derive(Copy, Clone)]
pub struct TableHandle<K, V> {
    table: Table,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> TableHandle<K, V> {
    pub const fn new(table: Table) -> Self {
        Self {
            table,
            _marker: PhantomData,
        }
    }

    pub const fn table(&self) -> Table {
        self.table
    }
}

impl<K, V> fmt::Debug for TableHandle<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableHandle")
            .field("table", &self.table)
            .finish()
    }
}

pub mod handles {
    use super::{Table, TableHandle};
    use wavs_types::{Service, ServiceId};

    pub const SERVICES: TableHandle<ServiceId, Service> = TableHandle::new(Table::Services);
    pub const SERVICES_BY_HASH: TableHandle<[u8; 32], Service> =
        TableHandle::new(Table::ServicesByHash);
    pub const AGGREGATOR_SERVICES: TableHandle<ServiceId, ()> =
        TableHandle::new(Table::AggregatorServices);
    pub const KV_STORE: TableHandle<String, Vec<u8>> = TableHandle::new(Table::KvStore);
    pub const KV_ATOMICS_COUNTER: TableHandle<String, i64> =
        TableHandle::new(Table::KvAtomicsCounter);
}

pub type DBError = anyhow::Error;

type AnyMap = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Default)]
pub struct WavsDb {
    tables: Arc<DashMap<Table, AnyMap>>,
}

impl WavsDb {
    #[instrument(fields(subsys = "WavsDb"))]
    pub fn new() -> Result<Self, DBError> {
        Ok(Self {
            tables: Arc::new(DashMap::new()),
        })
    }

    #[instrument(skip(self, key, value), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn set<K, V>(&self, handle: &TableHandle<K, V>, key: K, value: V) -> Result<(), DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let map = self.table_map(handle)?;
        map.insert(key, value);
        Ok(())
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn get<K, V>(&self, handle: &TableHandle<K, V>, key: &K) -> Result<Option<V>, DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let map = self.table_map(handle)?;
        Ok(map.get(key).map(|v| v.clone()))
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn remove<K, V>(&self, handle: &TableHandle<K, V>, key: &K) -> Result<Option<V>, DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let map = self.table_map(handle)?;
        Ok(map.remove(key).map(|(_, v)| v))
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn contains_key<K, V>(&self, handle: &TableHandle<K, V>, key: &K) -> Result<bool, DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let map = self.table_map(handle)?;
        Ok(map.contains_key(key))
    }

    #[instrument(skip(self), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn clear_table<K, V>(&self, handle: &TableHandle<K, V>) -> Result<(), DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let map = self.table_map(handle)?;
        map.clear();
        Ok(())
    }

    #[instrument(skip(self, f), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn with_table_read<K, V, F, R>(
        &self,
        handle: &TableHandle<K, V>,
        f: F,
    ) -> Result<R, DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
        F: FnOnce(&TableReadGuard<K, V>) -> Result<R, DBError>,
    {
        let map = self.table_map(handle)?;
        let guard = TableReadGuard { map };
        f(&guard)
    }

    fn table_map<K, V>(&self, handle: &TableHandle<K, V>) -> Result<Arc<DashMap<K, V>>, DBError>
    where
        K: Eq + Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        match self.tables.entry(handle.table()) {
            Entry::Occupied(entry) => {
                let existing = entry.get().clone();
                existing
                    .downcast::<DashMap<K, V>>()
                    .map_err(|_| anyhow!("table {:?} type mismatch", handle.table()))
            }
            Entry::Vacant(entry) => {
                let map: Arc<DashMap<K, V>> = Arc::new(DashMap::new());
                let erased: AnyMap = map.clone();
                entry.insert(erased);
                Ok(map)
            }
        }
    }
}

pub struct TableReadGuard<K, V> {
    map: Arc<DashMap<K, V>>,
}

impl<K, V> TableReadGuard<K, V>
where
    K: Eq + Hash,
{
    pub fn iter(&self) -> TableIter<'_, K, V> {
        TableIter {
            inner: self.map.iter(),
        }
    }
}

pub struct TableIter<'a, K, V> {
    inner: dashmap::iter::Iter<'a, K, V>,
}

impl<'a, K, V> Iterator for TableIter<'a, K, V>
where
    K: Eq + Hash,
{
    type Item = TableEntry<'a, K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(TableEntry)
    }
}

pub struct TableEntry<'a, K, V>(RefMulti<'a, K, V>);

impl<'a, K, V> TableEntry<'a, K, V>
where
    K: Eq + Hash,
{
    pub fn pair(&self) -> (&K, &V) {
        (self.0.key(), self.0.value())
    }
}

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
    fn set_get_round_trip() {
        let db = WavsDb::new().unwrap();
        let handle: TableHandle<u32, TestStruct> =
            TableHandle::new(Table::Test("test_u32_teststruct"));
        let key = 7u32;
        let value = TestStruct {
            name: "demo".to_string(),
            value: 99,
        };

        assert!(db.get(&handle, &key).unwrap().is_none());
        db.set(&handle, key, value.clone()).unwrap();
        assert_eq!(db.get(&handle, &key).unwrap(), Some(value));
    }

    #[test]
    fn remove_and_contains() {
        let db = WavsDb::new().unwrap();
        let handle: TableHandle<String, i64> = TableHandle::new(Table::KvAtomicsCounter);
        let key = "counter".to_string();

        assert!(!db.contains_key(&handle, &key).unwrap());
        db.set(&handle, key.clone(), 5).unwrap();
        assert!(db.contains_key(&handle, &key).unwrap());

        let removed = db.remove(&handle, &key).unwrap();
        assert_eq!(removed, Some(5));
        assert!(db.get(&handle, &key).unwrap().is_none());
    }

    #[test]
    fn table_iteration() {
        let db = WavsDb::new().unwrap();
        let handle: TableHandle<String, TestStruct> =
            TableHandle::new(Table::Test("test_string_teststruct"));
        db.set(
            &handle,
            "alpha".to_string(),
            TestStruct {
                name: "a".to_string(),
                value: 1,
            },
        )
        .unwrap();
        db.set(
            &handle,
            "beta".to_string(),
            TestStruct {
                name: "b".to_string(),
                value: 2,
            },
        )
        .unwrap();

        let mut seen = Vec::new();
        db.with_table_read(&handle, |table| {
            for entry in table.iter() {
                let (key, value) = entry.pair();
                seen.push((key.clone(), value.value));
            }
            Ok(())
        })
        .unwrap();

        seen.sort();
        assert_eq!(seen, vec![("alpha".into(), 1), ("beta".into(), 2)]);
    }
}
