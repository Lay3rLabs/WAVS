use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use opentelemetry::global;
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, setup_tracing, AggregatorMetrics},
};
use wavs_aggregator::{args::CliArgs, config::Config, run_server};

fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

    let ctx = AppContext::new();

    // setup tracing
    let filters = config.tracing_env_filter().unwrap();
    let tracer_provider = if let Some(collector) = config.jaeger.as_ref() {
        Some(ctx.rt.block_on({
            let config = config.clone();
            async move {
                setup_tracing(
                    collector,
                    "wavs-aggregator",
                    config.tracing_env_filter().unwrap(),
                )
            }
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
            "aggregator_metrics",
            config.prometheus_push_interval_secs,
        )
    });
    let meter = global::meter("aggregator_metrics");
    let metrics = AggregatorMetrics::new(meter);

    run_server(ctx, config, metrics);

    if let Some(meter) = meter_provider {
        meter
            .shutdown()
            .expect("MeterProvider should shutdown successfully");
    }

    if let Some(tracer) = tracer_provider {
        tracer
            .shutdown()
            .expect("TracerProvider should shutdown successfully")
    }
}
