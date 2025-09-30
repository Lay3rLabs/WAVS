use std::sync::Arc;

use clap::Parser;
use opentelemetry::global;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, setup_tracing, Metrics},
};
use wavs::{
    args::CliArgs,
    config::{Config, HealthCheckMode},
    dispatcher::Dispatcher,
    health::{create_shared_health_status, update_health_status},
};

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

    let health_status = create_shared_health_status();

    let chains = config.chains.all_chain_keys().unwrap();
    if !chains.is_empty() {
        match config.health_check_mode {
            HealthCheckMode::Bypass => {
                // Spawn background task to run health checks and log results
                let health_status_clone = health_status.clone();
                let chain_configs = config.chains.clone();
                ctx.rt.spawn(async move {
                    tracing::info!("Running health checks in background (bypass mode)");
                    if let Err(err) =
                        update_health_status(&health_status_clone, &chain_configs).await
                    {
                        tracing::warn!("Background health check failed: {}", err);
                    }
                });
            }
            HealthCheckMode::Wait => {
                // Run health checks and warn on failures
                ctx.rt.block_on(async {
                    if let Err(err) = update_health_status(&health_status, &config.chains).await {
                        tracing::warn!("Health check failed: {}", err);
                    }
                });
            }
            HealthCheckMode::Exit => {
                // Run health checks and panic on failures
                ctx.rt.block_on(async {
                    if let Err(err) = update_health_status(&health_status, &config.chains).await {
                        panic!("Health check failed (exit mode): {err}");
                    }
                });
            }
        }
    }

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

    wavs::run_server(ctx, config, dispatcher, metrics.http, health_status);

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
