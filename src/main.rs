use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{args::CliArgs, config::ConfigBuilder, dispatcher::core::CoreDispatcher, http};

fn main() {
    let args = CliArgs::parse();
    let config = ConfigBuilder::new(args).build().unwrap();

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

    let dispatcher = Arc::new(CoreDispatcher::new(config).unwrap());

    // start the http server in its own thread
    let server_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        move || {
            http::server::start(dispatcher).unwrap();
        }
    });

    let dispatcher_handle = std::thread::spawn({
        let dispatcher = dispatcher.clone();
        move || {
            dispatcher.start().unwrap();
        }
    });

    // wait for all threads to finish

    server_handle.join().unwrap();
    dispatcher_handle.join().unwrap();
}
