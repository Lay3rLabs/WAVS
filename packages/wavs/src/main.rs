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

fn setup_tracing(collector: &str, config: &Config) -> SdkTracerProvider {
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
    let tracer = provider.tracer("wavs-tracer");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = tracing_subscriber::Registry::default()
        .with(
            /*tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("trace".parse().unwrap()), */
            config.tracing_env_filter().unwrap(),
        )
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    tracing::info!("Jaeger tracing enabled");
    provider
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let config: Config = ConfigBuilder::new(args).build().unwrap();

    // setup tracing
    // if let Some(collector) = config.jaeger.as_ref() {
    let tracer_provider = setup_tracing("http://localhost:4317", &config);
    // } else {
    //     tracing_subscriber::registry()
    //         .with(
    //             tracing_subscriber::fmt::layer()
    //                 .without_time()
    //                 .with_target(false),
    //         )
    //         .with(config.tracing_env_filter().unwrap())
    //         .try_init()
    //         .unwrap();
    // }

    let tracer = tracer_provider.tracer("wavs-tracer");
    let root_span = tracing::span!(tracing::Level::INFO, "root-span");
    let _guard = root_span.enter(); // Enter the root span

    tracer.in_span("child_span_test", |cx| {
        tracing::info!("This is a trace log inside the span");
    });
    tracing::info!("This is a trace log outside the span");

    let ctx = AppContext::new();

    let config_clone = config.clone();
    let dispatcher = Arc::new(CoreDispatcher::new_core(&config_clone).unwrap());

    wavs::run_server(ctx, config, dispatcher);
    tracer_provider
        .shutdown()
        .expect("TracerProvider should shutdown successfully")
}
