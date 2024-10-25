use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{args::CliArgs, config::ConfigBuilder, http};

fn main() {
    let args = CliArgs::parse();
    let config = Arc::new(ConfigBuilder::new(args).build().unwrap());

    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter().unwrap())
        .try_init()
        .unwrap();

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let config = config.clone();
        move || {
            http::server::start(config).unwrap();
        }
    });

    // wait for the server to finish
    // TODO: add more thread handles here to wait on

    server_handle.join().unwrap();
}
