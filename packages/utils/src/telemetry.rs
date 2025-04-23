use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    resource::Resource,
    trace::{self, Sampler, SdkTracerProvider},
};
use tracing_subscriber::layer::SubscriberExt;

pub fn setup_tracing(
    collector: &str,
    service_name: &str,
    filters: tracing_subscriber::EnvFilter,
) -> SdkTracerProvider {
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
        .with_resource(
            Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .build();
    global::set_tracer_provider(provider.clone());
    let tracer = provider.tracer(format!("{}-tracer", service_name));
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = tracing_subscriber::Registry::default()
        .with(filters)
        .with(tracing_subscriber::fmt::layer()) // console logging layer
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default subscriber");

    tracing::info!("Jaeger tracing enabled");
    provider
}
