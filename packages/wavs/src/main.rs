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
    dispatcher::{Dispatcher, TauriHandle},
    health::SharedHealthStatus,
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

    let health_status = SharedHealthStatus::new();

    let (chains, chain_configs) = {
        let chain_configs = config.chains.read().unwrap().clone();
        let chains = chain_configs.all_chain_keys().unwrap();
        (chains, chain_configs)
    };
    if !chains.is_empty() {
        match config.health_check_mode {
            HealthCheckMode::Bypass => {
                let health_status_clone = health_status.clone();
                ctx.rt.spawn(async move {
                    tracing::info!("Running health checks in background (bypass mode)");
                    health_status_clone.update(&chain_configs).await;
                    if health_status_clone.any_failing() {
                        tracing::warn!(
                            "Health check failed: {:#?}",
                            health_status_clone.read().unwrap()
                        );
                    }
                });
            }
            HealthCheckMode::Wait => {
                ctx.rt.block_on(async {
                    health_status.update(&chain_configs).await;
                    if health_status.any_failing() {
                        tracing::warn!("Health check failed: {:#?}", health_status.read().unwrap());
                    }
                });
            }
            HealthCheckMode::Exit => {
                ctx.rt.block_on(async {
                    health_status.update(&chain_configs).await;
                    if health_status.any_failing() {
                        panic!(
                            "Health check failed (exit mode): {:#?}",
                            health_status.read().unwrap()
                        );
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
    let dispatcher =
        Arc::new(Dispatcher::new(&config_clone, metrics.wavs, TauriHandle::Mock).unwrap());

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
