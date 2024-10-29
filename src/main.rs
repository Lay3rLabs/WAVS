use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmatic::{
    args::CliArgs, config::ConfigBuilder, context::AppContext, dispatcher::CoreDispatcher,
};

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

    let ctx = AppContext::new(config);

    let dispatcher = Arc::new(CoreDispatcher::new_core(ctx.clone()).unwrap());

    wasmatic::start(ctx, dispatcher);
}
