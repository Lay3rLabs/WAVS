#![allow(clippy::result_large_err)]
#![allow(clippy::uninlined_format_args)]
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
