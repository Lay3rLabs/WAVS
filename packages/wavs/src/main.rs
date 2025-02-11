use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
};
use wavs::{args::CliArgs, config::Config, dispatcher::CoreDispatcher};

fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

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

    let ctx = AppContext::new();

    let config_clone = config.clone();
    let dispatcher = ctx.rt.block_on(async move {Arc::new(CoreDispatcher::new_core(&config_clone).await.unwrap())});

    wavs::run_server(ctx, config, dispatcher);
}
