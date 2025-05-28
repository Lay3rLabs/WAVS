use std::sync::Arc;

use clap::Parser;
use opentelemetry::global;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    health::health_check_chains_query,
    telemetry::{setup_metrics, setup_tracing, Metrics},
};
use wavs::{args::CliArgs, config::Config, dispatcher::Dispatcher};

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

    ctx.rt.block_on(async {
        // warn bad health for chains (services may or may not submit to these)
        let chain_names = config.chains.all_chain_names();
        if !chain_names.is_empty() {
            if let Err(err) = health_check_chains_query(&config.chains, &chain_names).await {
                tracing::warn!("Non-trigger-chain health-check failed: {}", err);
            }
        }
    });

    let meter_provider = config
        .prometheus
        .as_ref()
        .map(|collector| setup_metrics(collector, "wavs_metrics"));
    let meter = global::meter("wavs_metrics");
    let metrics = Metrics::new(&meter);

    let config_clone = config.clone();
    let dispatcher = Arc::new(Dispatcher::new(&config_clone, metrics.wavs).unwrap());

    wavs::run_server(ctx, config, dispatcher, metrics.http);
    if let Some(tracer) = tracer_provider {
        tracer
            .shutdown()
            .expect("TracerProvider should shutdown successfully")
    }
    if let Some(meter) = meter_provider {
        meter
            .shutdown()
            .expect("MeterProvider should shutdown successfully")
    }
}
