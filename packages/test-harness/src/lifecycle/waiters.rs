//! On-chain waiters: poll until a condition holds or a timeout elapses.
//!
//! Patterns copied from `packages/layer-tests/src/e2e/helpers.rs`
//! (`evm_wait_for_task_to_land`, `wait_for_evm_trigger_streams_to_finalize`) —
//! layer-tests has no `[lib]` target so wrapping is not currently possible.

use std::future::Future;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

/// Poll `f` until it returns `Some(value)` or `timeout` elapses.
///
/// `interval` controls poll cadence (default 250 ms if `None`).
pub async fn wait_for<T, F, Fut>(
    timeout: Duration,
    interval: Option<Duration>,
    mut f: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let interval = interval.unwrap_or(Duration::from_millis(250));
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = f().await {
            return Ok(v);
        }
        if Instant::now() >= deadline {
            return Err(anyhow!("timed out after {:?}", timeout));
        }
        tokio::time::sleep(interval).await;
    }
}

/// Poll `f` until it returns `Ok(value)` matching `predicate`, or `timeout` elapses.
///
/// Errors from `f` are swallowed (treated as "not yet ready") so that transient
/// RPC failures don't break the loop.
pub async fn wait_until<T, F, Fut, P>(
    timeout: Duration,
    interval: Option<Duration>,
    mut f: F,
    predicate: P,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
    P: Fn(&T) -> bool,
{
    let interval = interval.unwrap_or(Duration::from_millis(250));
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(v) = f().await {
            if predicate(&v) {
                return Ok(v);
            }
        }
        if Instant::now() >= deadline {
            return Err(anyhow!("predicate not satisfied within {:?}", timeout));
        }
        tokio::time::sleep(interval).await;
    }
}
