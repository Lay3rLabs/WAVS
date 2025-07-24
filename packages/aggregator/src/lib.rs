#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::result_large_err)]
pub mod args;
pub mod config;
pub mod engine;
pub mod error;
pub mod http;

use tracing::instrument;
use utils::context::AppContext;

/// Entry point to start up the server
/// Called from main
#[instrument(level = "info", skip(ctx, config))]
pub fn run_server(ctx: AppContext, config: config::Config) {
    tracing::info!(
        "Aggregator server initializing with data path: {:?}",
        config.data
    );

    let _ = ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    });

    let server_handle = std::thread::spawn({
        let ctx = ctx.clone();
        move || {
            tracing::info!("Starting HTTP server thread");
            http::server::start(ctx.clone(), config).unwrap();
        }
    });

    tracing::info!("Waiting for server thread to complete");
    server_handle.join().unwrap();
    tracing::info!("Aggregator server shutdown complete");
}

// the test version of init_tracing does not take a config
// since config itself is tested and modified from different parallel tests
// therefore, this only uses the default tracing settings
// it's not gated out because it is used in benches and integration tests as well
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
