use std::sync::Arc;

use clap::Parser;
use opentelemetry::{
    global,
    trace::{Span, Tracer, TracerProvider as _},
};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    resource::Resource,
    trace::{self, Sampler, SdkTracerProvider},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigBuilder, ConfigExt},
    context::AppContext,
};
use wavs::{args::CliArgs, config::Config, dispatcher::CoreDispatcher};

fn setup_tracing(collector: &str, config: &Config) {
    global::set_text_map_propagator(opentelemetry_jaeger_propagator::Propagator::new());
    let endpoint = format!("{}/v1/traces", collector);
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
    let tracer = provider.tracer("readme_example");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = tracing_subscriber::Registry::default()
        .with(config.tracing_env_filter().unwrap())
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    tracing::info!("Jaeger tracing enabled");
}

fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

    // setup tracing
    if let Some(collector) = config.jaeger.as_ref() {
        setup_tracing(collector, &config);
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_target(false),
            )
            .with(config.tracing_env_filter().unwrap())
            .try_init()
            .unwrap();
    }

    let tracer = opentelemetry::global::tracer("example-tracer");
    let mut span = tracer.start("main-span");
    span.set_attribute(opentelemetry::KeyValue::new("key", "value"));
    {
        span.add_event(
            "processing-started",
            vec![
                opentelemetry::KeyValue::new("event_key", "event_value"),
                opentelemetry::KeyValue::new("status", "started"),
            ],
        );
    }

    let ctx = AppContext::new();

    let config_clone = config.clone();
    let dispatcher = Arc::new(CoreDispatcher::new_core(&config_clone).unwrap());

    wavs::run_server(ctx, config, dispatcher);
}
