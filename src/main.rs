use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{args::CliArgs, config::ConfigBuilder, dispatcher::core::CoreDispatcher};

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

    wasmatic::start(dispatcher);
}
