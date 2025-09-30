#![allow(clippy::result_large_err)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]

pub mod args;
pub mod config;
pub mod dispatcher; // where we have the high-level dispatcher
pub mod health;
pub mod http;
pub mod services;
pub mod subsystems; // subsystems: engine, submission, and trigger // services lookup

use config::Config;
use dispatcher::Dispatcher;
use health::SharedHealthStatus;
use utils::storage::fs::FileStorage;

// This section is called from both main and end-to-end tests
use std::sync::Arc;
use utils::context::AppContext;
use utils::telemetry::HttpMetrics;

/// Entry point to start up the whole server
/// Called from main and end-to-end tests
pub fn run_server(
    ctx: AppContext,
    config: Config,
    dispatcher: Arc<Dispatcher<FileStorage>>,
    metrics: HttpMetrics,
    health_status: SharedHealthStatus,
) {
    let _ = ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    });

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        let ctx = ctx.clone();
        move || {
            http::server::start(ctx, config, dispatcher, metrics, health_status).unwrap();
        }
    });

    let dispatcher_handle = std::thread::spawn(move || {
        dispatcher.start(ctx).unwrap();
    });

    // wait for all threads to finish

    server_handle.join().unwrap();
    dispatcher_handle.join().unwrap();
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
