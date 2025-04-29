//! An async, per‑key transactional executor.
//!
//! ## Why not `DashMap`?
//! * **Sharding vs. per‑key isolation** – `DashMap` locks buckets; different
//!   keys might share a shard and therefore serialize.  We want *exact* key‑level
//!   exclusivity.
//! * **Async‑friendliness** – `DashMap` is sync; mixing it with `.await` is easy
//!   foot‑gun territory, typically resulting in deadlocks.  Here the critical section
//!   uses `tokio::sync::Mutex`, so holding it across `.await` is sound.
//!
//! ## Design
//! * A global **registry** (`tokio::sync::RwLock<HashMap<K, Arc<Mutex<()>>>>`)
//!   maps every key to its own async mutex.
//! * The registry uses **`tokio::RwLock`** – completely non‑blocking inside an
//!   async context.  We *never* hold the registry lock across `.await` points.
//! * The executor is **`Clone`** (cheap `Arc` clone) so each spawned task can
//!   own it.
//! * **Optional cleanup** – disabled by default; if enabled, we prune a key when
//!   its mutex’s last `Arc` drops (requires a write‑lock).
//!
//! ## Example
//! ```ignore
//! let exec = TransactionExecutor::new(false); // no cleanup
//! exec.run_transaction("user:42", || async {
//!     // exclusive region for "user:42"
//! }).await;
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// An async executor that guarantees **one transaction at a time per key**.
#[derive(Clone)]
pub struct AsyncTransaction<K: Eq + Hash + Clone> {
    locks: Arc<RwLock<HashMap<K, Arc<Mutex<()>>>>>,
    cleanup_enabled: bool,
}

impl<K: Eq + Hash + Clone> AsyncTransaction<K> {
    /// Create a new executor.
    ///
    /// * `cleanup_enabled` – if `true`, unused per‑key locks are removed after a
    ///   transaction (incurs an extra write‑lock).
    pub fn new(cleanup_enabled: bool) -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            cleanup_enabled,
        }
    }

    /// Run `f` as an **exclusive async transaction** for `key`.
    pub async fn run<F, Fut, T>(&self, key: K, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        // Acquire (or create) the per‑key mutex.
        let lock_arc = {
            let read = self.locks.read().await;
            if let Some(lock) = read.get(&key) {
                lock.clone()
            } else {
                drop(read); // release read before write
                let mut write = self.locks.write().await;
                write
                    .entry(key.clone())
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone()
            }
        };

        // Critical section for this key.
        let out = {
            let _guard = lock_arc.lock().await;
            f().await
        };

        // Optional pruning of unused lock.
        // it's exactly 2 because 1 for the map entry + 1 for this local clone
        // any other transaction will also increase by 2 at a time, we don't hand out the lock
        if self.cleanup_enabled && Arc::strong_count(&lock_arc) == 2 {
            let mut write = self.locks.write().await;
            write.remove(&key);
        }

        out
    }

    /// Explicitly remove a key's mutex from the registry.
    /// Returns `true` if a lock was removed. In‑flight transactions are safe.
    pub async fn remove_key(&self, key: &K) -> bool {
        self.locks.write().await.remove(key).is_some()
    }

    /// Get a list of all keys in the registry.
    pub async fn clone_keys(&self) -> Vec<K> {
        let read = self.locks.read().await;
        read.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Launch `n` concurrent transactions
    async fn max_concurrent(
        exe: AsyncTransaction<&'static str>,
        keys: Vec<&'static str>,
        n: usize,
    ) -> usize {
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..n {
            for key in keys.clone() {
                let exe_cloned = exe.clone();
                let cur = current.clone();
                let pk = peak.clone();
                handles.push(tokio::spawn(async move {
                    exe_cloned
                        .run(key, || async move {
                            let now = cur.fetch_add(1, Ordering::SeqCst) + 1;
                            pk.fetch_max(now, Ordering::SeqCst);
                            // Simulate some work
                            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                            cur.fetch_sub(1, Ordering::SeqCst);
                        })
                        .await;
                }));
            }
        }

        futures::future::join_all(handles).await;
        peak.load(Ordering::SeqCst)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn single_key_multi_thread() {
        let exe = AsyncTransaction::new(false);
        let exe_1 = exe.clone();
        let exe_2 = exe.clone();

        // Even though we launch a bunch of tasks for different keys,
        // Each one gets its own lock
        let p1 = tokio::spawn(async move { max_concurrent(exe_1, vec!["foo"], 20).await });
        let p2 = tokio::spawn(async move { max_concurrent(exe_2, vec!["bar"], 20).await });
        let (peak1, peak2) = tokio::join!(p1, p2);

        // each key ran serially
        assert_eq!(peak1.unwrap(), 1);
        assert_eq!(peak2.unwrap(), 1);

        // sanity check, did not auto-prune
        let mut keys = exe.clone_keys().await;
        keys.sort();
        assert_eq!(keys, vec!["bar", "foo"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_key_multi_thread() {
        let exe = AsyncTransaction::new(false);
        let exe_1 = exe.clone();
        let exe_2 = exe.clone();

        // Even though we launch a bunch of tasks for different keys,
        // Each one gets its own lock
        let p1 = tokio::spawn(async move {
            max_concurrent(exe_1, vec!["foo_1", "foo_2", "foo_3"], 20).await
        });
        let p2 = tokio::spawn(async move {
            max_concurrent(exe_2, vec!["bar_1", "bar_2", "bar_3"], 20).await
        });
        let (peak1, peak2) = tokio::join!(p1, p2);

        // each key ran serially - but did not block each other
        assert_eq!(peak1.unwrap(), 3);
        assert_eq!(peak2.unwrap(), 3);

        // sanity check, did not auto-prune
        let mut keys = exe.clone_keys().await;
        keys.sort();
        assert_eq!(
            keys,
            vec!["bar_1", "bar_2", "bar_3", "foo_1", "foo_2", "foo_3"]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn single_key_single_thread() {
        let exe = AsyncTransaction::new(false);
        let exe_1 = exe.clone();
        let exe_2 = exe.clone();

        // Even though we launch a bunch of tasks for different keys,
        // Each one gets its own lock
        let p1 = tokio::spawn(async move { max_concurrent(exe_1, vec!["foo"], 20).await });
        let p2 = tokio::spawn(async move { max_concurrent(exe_2, vec!["bar"], 20).await });
        let (peak1, peak2) = tokio::join!(p1, p2);

        // each key ran serially
        assert_eq!(peak1.unwrap(), 1);
        assert_eq!(peak2.unwrap(), 1);

        // sanity check, did not auto-prune
        let mut keys = exe.clone_keys().await;
        keys.sort();
        assert_eq!(keys, vec!["bar", "foo"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn multi_key_single_thread() {
        let exe = AsyncTransaction::new(false);
        let exe_1 = exe.clone();
        let exe_2 = exe.clone();

        // Even though we launch a bunch of tasks for different keys,
        // Each one gets its own lock
        let p1 = tokio::spawn(async move {
            max_concurrent(exe_1, vec!["foo_1", "foo_2", "foo_3"], 20).await
        });
        let p2 = tokio::spawn(async move {
            max_concurrent(exe_2, vec!["bar_1", "bar_2", "bar_3"], 20).await
        });
        let (peak1, peak2) = tokio::join!(p1, p2);

        // each key ran serially - but did not block each other
        assert_eq!(peak1.unwrap(), 3);
        assert_eq!(peak2.unwrap(), 3);

        // sanity check, did not auto-prune
        let mut keys = exe.clone_keys().await;
        keys.sort();
        assert_eq!(
            keys,
            vec!["bar_1", "bar_2", "bar_3", "foo_1", "foo_2", "foo_3"]
        );
    }

    #[tokio::test]
    async fn remove_key() {
        let exe = AsyncTransaction::new(false);
        exe.run("temp", || async {}).await;
        assert!(exe.remove_key(&"temp").await);
        assert!(!exe.remove_key(&"temp").await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn auto_prune() {
        let exe = AsyncTransaction::new(true);
        let exe_1 = exe.clone();
        let exe_2 = exe.clone();

        // Even though we launch a bunch of tasks for different keys,
        // Each one gets its own lock
        let p1 = tokio::spawn(async move {
            max_concurrent(exe_1, vec!["foo_1", "foo_2", "foo_3"], 20).await
        });
        let p2 = tokio::spawn(async move {
            max_concurrent(exe_2, vec!["bar_1", "bar_2", "bar_3"], 20).await
        });
        let (peak1, peak2) = tokio::join!(p1, p2);

        // each key ran serially - but did not block each other
        assert_eq!(peak1.unwrap(), 3);
        assert_eq!(peak2.unwrap(), 3);

        // sanity check, did auto-prune
        let keys = exe.clone_keys().await;
        assert!(keys.is_empty());
    }
}
