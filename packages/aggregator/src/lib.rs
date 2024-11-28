pub mod args;
pub mod config;
pub mod context;
pub mod http;
pub mod test_utils;

use context::AppContext;

/// Entry point to start up the server
/// Called from main
pub fn run_server(ctx: AppContext, config: config::Config) {
    ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    })
    .unwrap();

    // start the http server in its own thread
    http::server::start(ctx, config).unwrap();
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
