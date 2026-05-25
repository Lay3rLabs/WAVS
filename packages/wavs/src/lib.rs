#![allow(clippy::result_large_err)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::type_complexity)]

pub mod args;
pub mod config;
pub mod dispatcher; // where we have the high-level dispatcher
pub mod health;
pub mod http;
pub mod log_buffer;
pub mod service_registry;
pub mod services;
pub mod subsystems; // subsystems: engine, submission, and trigger // services lookup

use config::Config;
use dispatcher::Dispatcher;
use health::SharedHealthStatus;
use log_buffer::LogBuffer;
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
    log_buffer: LogBuffer,
) {
    let _ = ctrlc::set_handler({
        let ctx = ctx.clone();
        move || {
            ctx.kill();
        }
    });

    // Channel used to delay the dispatcher restore loop until the HTTP server is bound.
    // This prevents "connection refused" errors when restoring dev-service URIs on restart.
    let (http_ready_tx, http_ready_rx) = std::sync::mpsc::sync_channel::<()>(1);

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        let ctx = ctx.clone();
        move || {
            http::server::start(
                ctx,
                config,
                dispatcher,
                metrics,
                health_status,
                log_buffer,
                http_ready_tx,
            )
            .unwrap();
        }
    });

    let dispatcher_handle = std::thread::spawn(move || {
        // Wait until the HTTP server has bound its port before running the restore loop,
        // so that fetches of local dev-service URIs don't get "connection refused".
        let _ = http_ready_rx.recv();
        dispatcher.start(ctx).unwrap();
    });

    // wait for all threads to finish

    if let Err(e) = server_handle.join() {
        tracing::error!("HTTP server thread panicked: {:?}", e);
    }
    if let Err(e) = dispatcher_handle.join() {
        tracing::error!("Dispatcher thread panicked: {:?}", e);
    }
}
