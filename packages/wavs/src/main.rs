use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::setup_tracing,
};
use wavs::{args::CliArgs, config::Config, dispatcher::CoreDispatcher};

fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

    let ctx = AppContext::new();

    // setup tracing
    let filters = config.tracing_env_filter().unwrap();
    let tracer_provider = if let Some(collector) = config.jaeger.as_ref() {
        Some(ctx.rt.block_on({
            let config = config.clone();
            async move { setup_tracing(collector, "wavs", config.tracing_env_filter().unwrap()) }
        }))
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_target(false),
            )
            .with(filters)
            .try_init()
            .unwrap();
        None
    };

    let config_clone = config.clone();
    let dispatcher = Arc::new(CoreDispatcher::new_core(&config_clone).unwrap());

    wavs::run_server(ctx, config, dispatcher);
    if let Some(tracer) = tracer_provider {
        tracer
            .shutdown()
            .expect("TracerProvider should shutdown successfully")
    }
}
