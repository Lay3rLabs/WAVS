use std::num::NonZeroUsize;
use std::sync::Mutex;

use lru::LruCache;
use serde_json::Value;
use wavs_types::ComponentDigest;

const DEFAULT_CACHE_SIZE: usize = 32;

/// LRU cache for generated schemas, keyed by component digest (SHA256).
///
/// Thread-safe via Mutex, following the same pattern as `BaseEngine` in
/// `packages/engine/src/common/base_engine.rs`.
pub struct SchemaCache {
    cache: Mutex<LruCache<ComponentDigest, Value>>,
}

impl SchemaCache {
    /// Create a new cache with the given capacity.
    /// If capacity is 0, falls back to DEFAULT_CACHE_SIZE.
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity)
                    .unwrap_or(NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap()),
            )),
        }
    }

    /// Look up a cached schema by component digest.
    /// Returns a clone of the cached value if found.
    pub fn get(&self, digest: &ComponentDigest) -> Option<Value> {
        self.cache.lock().unwrap().get(digest).cloned()
    }

    /// Store a schema in the cache, keyed by component digest.
    pub fn put(&self, digest: ComponentDigest, schema: Value) {
        self.cache.lock().unwrap().put(digest, schema);
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_digest(data: &[u8]) -> ComponentDigest {
        ComponentDigest::hash(data)
    }

    #[test]
    fn test_put_then_get_returns_same_value() {
        let cache = SchemaCache::default();
        let digest = make_digest(b"test-component-bytes");
        let schema = json!({"world": "test", "exports": {}});

        cache.put(digest.clone(), schema.clone());
        let result = cache.get(&digest);

        assert_eq!(result, Some(schema));
    }

    #[test]
    fn test_get_missing_key_returns_none() {
        let cache = SchemaCache::default();
        let digest = make_digest(b"nonexistent");

        assert_eq!(cache.get(&digest), None);
    }

    #[test]
    fn test_cache_eviction_when_capacity_exceeded() {
        let cache = SchemaCache::new(2);

        let d1 = make_digest(b"component-1");
        let d2 = make_digest(b"component-2");
        let d3 = make_digest(b"component-3");

        cache.put(d1.clone(), json!({"id": 1}));
        cache.put(d2.clone(), json!({"id": 2}));
        // This should evict d1
        cache.put(d3.clone(), json!({"id": 3}));

        assert_eq!(cache.get(&d1), None, "d1 should have been evicted");
        assert_eq!(cache.get(&d2), Some(json!({"id": 2})));
        assert_eq!(cache.get(&d3), Some(json!({"id": 3})));
    }

    #[test]
    fn test_default_creates_cache_with_capacity_32() {
        let cache = SchemaCache::default();
        // We can verify by inserting 32 items and checking they're all still there
        for i in 0..32 {
            let digest = make_digest(format!("component-{}", i).as_bytes());
            cache.put(digest, json!({"id": i}));
        }
        // All 32 should be present
        for i in 0..32 {
            let digest = make_digest(format!("component-{}", i).as_bytes());
            assert!(
                cache.get(&digest).is_some(),
                "component-{} should be in cache",
                i
            );
        }
        // Adding a 33rd should evict the first
        let d33 = make_digest(b"component-32");
        cache.put(d33, json!({"id": 32}));
        let d0 = make_digest(b"component-0");
        assert_eq!(cache.get(&d0), None, "component-0 should have been evicted");
    }
}
