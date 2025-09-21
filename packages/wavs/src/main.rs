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

    let meter_provider = config.prometheus.as_ref().map(|collector| {
        setup_metrics(
            collector,
            "wavs_metrics",
            config.prometheus_push_interval_secs,
        )
    });
    let meter = global::meter("wavs_metrics");
    let metrics = Metrics::new(meter);

    let config_clone = config.clone();
    let dispatcher = Arc::new(Dispatcher::new(&config_clone, metrics.wavs).unwrap());

    let health_check_handle = std::thread::spawn({
        let config_chains = config.chains.clone();
        move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let chains = config_chains.all_chain_keys().unwrap();
                if !chains.is_empty() {
                    if let Err(err) = health_check_chains_query(&config_chains, &chains).await {
                        tracing::warn!("Non-trigger-chain health-check failed: {}", err);
                    }
                }
            });
        }
    });

    wavs::run_server(ctx, config, dispatcher, metrics.http);

    // Wait for health check to complete
    health_check_handle.join().unwrap();

    if let Some(tracer) = tracer_provider {
        if tracer.shutdown().is_err() {
            //eprintln!("TracerProvider didn't shutdown cleanly: {e:?}")
        }
    }
    if let Some(meter) = meter_provider {
        if meter.shutdown().is_err() {
            //eprintln!("MeterProvider didn't shutdown cleanly: {e:?}")
        }
    }
}
