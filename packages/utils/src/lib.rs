#![allow(clippy::uninlined_format_args)]

pub mod alloy_helpers;
pub mod async_transaction;
pub mod config;
pub mod context;
pub mod error;
pub mod evm_client;
pub mod filesystem;
pub mod health;
pub mod serde;
pub mod service;
pub mod storage;
pub mod telemetry;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
pub mod wkg;

// the test version of init_tracing does not take a config
// since config itself is tested and modified from different parallel tests
// therefore, this only uses the default tracing settings
// it's not gated out because it is used in benches and integration tests as well
#[cfg(debug_assertions)]
pub fn init_tracing_tests() {
    use std::sync::LazyLock;

    // however, it has an extra complexity of race conditions across threads
    // so we use a Mutex to ensure we only initialize once globally
    static INIT: LazyLock<std::sync::Mutex<bool>> = LazyLock::new(|| std::sync::Mutex::new(false));

    let mut init = INIT.lock().unwrap();

    if !*init {
        *init = true;

        // we want to be able to see tracing info in tests
        // also, although we could technically just store a separate tracing handle in each app
        // this serves as a good sanity check that we're only initializing once
        tracing_subscriber::fmt::init();
        tracing::debug!("Tracing initialized for tests");
    }
}
