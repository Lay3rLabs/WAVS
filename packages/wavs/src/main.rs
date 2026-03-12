use std::sync::Arc;

use clap::Parser;
use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    resource::Resource,
    trace::{self, Sampler, SdkTracerProvider},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
    telemetry::{setup_metrics, Metrics},
};
use wavs::{
    args::CliArgs,
    config::{Config, HealthCheckMode},
    dispatcher::{Dispatcher, TauriHandle},
    health::SharedHealthStatus,
    log_buffer::{InMemoryLogLayer, LogBufferInner},
};

fn main() {
    let args = CliArgs::parse();
    let mut config: Config = ConfigBuilder::new(args).build().unwrap();

    // Normalize empty credentials to None (e.g. env vars set to "" are treated as unset)
    config.normalize_credentials();

    let ctx = AppContext::new();

    // setup tracing
    let filters = config.tracing_env_filter().unwrap();
    let log_buffer =
        LogBufferInner::with_capacity(config.log_buffer_capacity, config.log_broadcast_capacity);
    let tracer_provider = if let Some(collector) = config.jaeger.as_ref() {
        global::set_text_map_propagator(opentelemetry_jaeger_propagator::Propagator::new());
        let endpoint = format!("{collector}/v1/traces");
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("Failed to build OTLP exporter");
        let batch_processor = trace::BatchSpanProcessor::builder(exporter).build();
        let provider = SdkTracerProvider::builder()
            .with_span_processor(batch_processor)
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(Resource::builder().with_service_name("wavs").build())
            .build();
        global::set_tracer_provider(provider.clone());
        let tracer = provider.tracer("wavs-tracer");
        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        tracing_subscriber::registry()
            .with(filters)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_line_number(true)
                    .compact(),
            )
            .with(telemetry)
            .with(InMemoryLogLayer::new(log_buffer.clone()))
            .try_init()
            .unwrap();
        tracing::info!("Jaeger tracing enabled");
        Some(provider)
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_target(false)
                    .compact(),
            )
            .with(filters)
            .with(InMemoryLogLayer::new(log_buffer.clone()))
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

    // Log aggregator credential status at startup
    let has_aggregator_cosmos = config.aggregator_cosmos_credential.is_some();
    let has_aggregator_evm = config.aggregator_evm_credential.is_some();

    if has_aggregator_cosmos || has_aggregator_evm {
        tracing::info!(
            "Aggregator credentials configured: Cosmos={}, EVM={}",
            has_aggregator_cosmos,
            has_aggregator_evm
        );
    } else {
        tracing::info!("No aggregator credentials configured - will not submit to chains");
    }

    let config_clone = config.clone();
    let dispatcher =
        Arc::new(Dispatcher::new(&config_clone, metrics.wavs, TauriHandle::Mock).unwrap());

    wavs::run_server(
        ctx,
        config,
        dispatcher,
        metrics.http,
        health_status,
        log_buffer,
    );

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
