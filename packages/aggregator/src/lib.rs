pub mod args;
pub mod config;
pub mod http;
pub mod test_utils;

use tokio::sync::broadcast::Receiver;
pub use utils::context::AppContext;

/// Entry point to start up the server
/// Called from main
pub fn run_server(ctx: AppContext, config: config::Config) {
    let _ = ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    });

    // Create a future that completes when kill signal is received
    let mut kill_signal: Receiver<()> = ctx.get_kill_receiver();

    // Start the http server with shutdown signal
    ctx.rt.block_on(async {
        let server = http::server::start(ctx.clone(), config)?;

        // Wait for either server error or kill signal
        tokio::select! {
            _ = kill_signal.recv() => {
                tracing::info!("Aggregator received shutdown signal");
                Ok(())
            }
            result = server => {
                match result {
                    Ok(inner_result) => inner_result,
                    Err(e) => {
                        tracing::error!("Server join error: {}", e);
                        Err(anyhow::anyhow!("Server join error: {}", e))
                    }
                }
            }
        }
    }).expect("Runtime error");
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
