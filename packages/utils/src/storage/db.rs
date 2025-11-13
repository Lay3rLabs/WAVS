use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use dashmap::mapref::entry::Entry;
use dashmap::mapref::multiple::RefMulti;
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json;
use tracing::instrument;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    Services,
    ServicesByHash,
    AggregatorServices,
    QuorumQueues,
    KvStore,
    KvAtomicsCounter,
    // Test tables
    TestU32String,
    TestStrDemo,
    TestTempServices,
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
            Table::TestU32String => "t1",
            Table::TestStrDemo => "tj",
            Table::TestTempServices => "temp-services",
        }
    }
}

#[derive(Copy, Clone)]
pub struct TableHandle<K, V> {
    table: Table,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> fmt::Debug for TableHandle<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableHandle")
            .field("table", &self.table)
            .finish()
    }
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

pub mod handles {
    use super::{Table, TableHandle};
    use wavs_types::Service;

    pub const SERVICES: TableHandle<[u8; 32], Service> = TableHandle::new(Table::Services);
    pub const SERVICES_BY_HASH: TableHandle<[u8; 32], Service> =
        TableHandle::new(Table::ServicesByHash);
    pub const AGGREGATOR_SERVICES: TableHandle<[u8; 32], ()> =
        TableHandle::new(Table::AggregatorServices);
    pub const KV_STORE: TableHandle<String, Vec<u8>> = TableHandle::new(Table::KvStore);
    pub const KV_ATOMICS_COUNTER: TableHandle<String, i64> =
        TableHandle::new(Table::KvAtomicsCounter);
}

pub type DBError = anyhow::Error;

type RawKey = Vec<u8>;
type RawValue = Vec<u8>;
type TableMap = DashMap<RawKey, RawValue>;

/// Multi-table in-memory DB with JSON serialization for typed callers.
#[derive(Clone, Default)]
pub struct WavsDb {
    tables: Arc<DashMap<Table, Arc<TableMap>>>,
}

impl WavsDb {
    #[instrument(fields(subsys = "WavsDb"))]
    pub fn new() -> Result<Self, DBError> {
        Ok(Self {
            tables: Arc::new(DashMap::new()),
        })
    }

    #[instrument(skip(self, key, value), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn set<K, V>(&self, handle: TableHandle<K, V>, key: K, value: &V) -> Result<(), DBError>
    where
        K: Serialize,
        V: Serialize,
    {
        let key_bytes = serde_json::to_vec(&key)?;
        let value_bytes = serde_json::to_vec(value)?;
        self.table(handle.table()).insert(key_bytes, value_bytes);
        Ok(())
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn get<K, V>(&self, handle: TableHandle<K, V>, key: K) -> Result<Option<V>, DBError>
    where
        K: Serialize,
        V: DeserializeOwned,
    {
        let key_bytes = serde_json::to_vec(&key)?;
        let table = match self.try_table(handle.table()) {
            Some(inner) => inner,
            None => return Ok(None),
        };

        Ok(table
            .get(&key_bytes)
            .map(|value| serde_json::from_slice::<V>(value.value()))
            .transpose()?)
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn remove<K, V>(&self, handle: TableHandle<K, V>, key: K) -> Result<Option<V>, DBError>
    where
        K: Serialize,
        V: DeserializeOwned,
    {
        let key_bytes = serde_json::to_vec(&key)?;
        let table = match self.try_table(handle.table()) {
            Some(inner) => inner,
            None => return Ok(None),
        };

        Ok(table
            .remove(&key_bytes)
            .map(|(_, value)| serde_json::from_slice::<V>(&value))
            .transpose()?)
    }

    #[instrument(skip(self, key), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn contains_key<K, V>(&self, handle: TableHandle<K, V>, key: K) -> Result<bool, DBError>
    where
        K: Serialize,
    {
        let key_bytes = serde_json::to_vec(&key)?;
        Ok(self
            .try_table(handle.table())
            .map(|inner| inner.contains_key(&key_bytes))
            .unwrap_or(false))
    }

    #[instrument(skip(self), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn clear_table<K, V>(&self, handle: TableHandle<K, V>) -> Result<(), DBError> {
        if let Some(inner) = self.try_table(handle.table()) {
            inner.clear();
        }
        Ok(())
    }

    /// Provide an immutable view into a table's serialized bytes.
    #[instrument(skip(self, f), fields(subsys = "WavsDb", table = ?handle.table()))]
    pub fn with_table_read<K, V, F, R>(&self, handle: TableHandle<K, V>, f: F) -> Result<R, DBError>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
        F: for<'a> FnOnce(&TableReadGuard<'a>) -> Result<R, DBError>,
    {
        let _ = PhantomData::<(K, V)>;
        let table_arc = self.table(handle.table());
        let result = {
            let guard = TableReadGuard::new(&table_arc);
            f(&guard)
        };
        result
    }

    fn table(&self, table: Table) -> Arc<TableMap> {
        match self.tables.entry(table) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let table = Arc::new(DashMap::new());
                entry.insert(table.clone());
                table
            }
        }
    }

    fn try_table(&self, table: Table) -> Option<Arc<TableMap>> {
        self.tables.get(&table).map(|entry| entry.clone())
    }
}

pub struct TableReadGuard<'a> {
    table: &'a TableMap,
}

impl<'a> TableReadGuard<'a> {
    fn new(table: &'a TableMap) -> Self {
        Self { table }
    }

    pub fn iter(&'a self) -> TableIter<'a> {
        TableIter {
            inner: self.table.iter(),
        }
    }
}

pub struct TableIter<'a> {
    inner: dashmap::iter::Iter<'a, RawKey, RawValue>,
}

impl<'a> Iterator for TableIter<'a> {
    type Item = TableEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(TableEntry)
    }
}

pub struct TableEntry<'a>(RefMulti<'a, RawKey, RawValue>);

impl<'a> TableEntry<'a> {
    pub fn pair(&self) -> (&[u8], &[u8]) {
        (self.0.key().as_slice(), self.0.value().as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use serde_json;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn set_get_round_trip() {
        let db = WavsDb::new().unwrap();
        let key = 7u32;
        let value = TestStruct {
            name: "demo".to_string(),
            value: 99,
        };
        const HANDLE: TableHandle<u32, TestStruct> = TableHandle::new(Table::TestU32String);

        assert!(db.get(HANDLE, key).unwrap().is_none());

        db.set(HANDLE, key, &value).unwrap();

        let round_trip = db.get(HANDLE, key).unwrap().unwrap();
        assert_eq!(round_trip, value);
    }

    #[test]
    fn remove_and_contains() {
        let db = WavsDb::new().unwrap();
        let key = "counter1".to_string();
        const HANDLE: TableHandle<String, i64> = TableHandle::new(Table::KvAtomicsCounter);

        assert!(!db.contains_key(HANDLE, key.clone()).unwrap());

        db.set(HANDLE, key.clone(), &41i64).unwrap();
        assert!(db.contains_key(HANDLE, key.clone()).unwrap());

        let removed = db.remove(HANDLE, key.clone()).unwrap().unwrap();
        assert_eq!(removed, 41);
        assert!(db.get(HANDLE, key).unwrap().is_none());
    }

    #[test]
    fn table_iteration_surface_raw_bytes() {
        let db = WavsDb::new().unwrap();
        const HANDLE: TableHandle<String, TestStruct> = TableHandle::new(Table::TestStrDemo);
        db.set(
            HANDLE,
            "alpha".to_string(),
            &TestStruct {
                name: "a".to_string(),
                value: 1,
            },
        )
        .unwrap();
        db.set(
            HANDLE,
            "beta".to_string(),
            &TestStruct {
                name: "b".to_string(),
                value: 2,
            },
        )
        .unwrap();

        let mut seen = Vec::new();
        db.with_table_read(HANDLE, |table| {
            for entry in table.iter() {
                let (key_bytes, value_bytes) = entry.pair();
                let key: String = serde_json::from_slice(key_bytes)?;
                let value: TestStruct = serde_json::from_slice(value_bytes)?;
                seen.push((key, value));
            }
            Ok(())
        })
        .unwrap();

        seen.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, "alpha");
        assert_eq!(seen[1].0, "beta");
    }
}
