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

use opentelemetry::metrics::{Counter, Gauge, Meter, UpDownCounter};

pub trait Metrics {
    fn init(meter: &Meter) -> Self;
}

#[derive(Clone)]
pub struct HttpMetrics {
    pub registered_services: UpDownCounter<i64>,
}

impl Metrics for HttpMetrics {
    fn init(meter: &Meter) -> Self {
        HttpMetrics {
            registered_services: meter
                .i64_up_down_counter("registered_services")
                .with_description("Number of services currently registered")
                .build(),
        }
    }
}

impl HttpMetrics {
    pub fn increment_registered_services(&self) {
        self.registered_services.add(1, &[]);
    }

    pub fn decrement_registered_services(&self) {
        self.registered_services.add(-1, &[]);
    }
}

#[derive(Clone)]
pub struct WavsMetrics {
    pub total_messages_processed: Counter<u64>,
    pub total_errors: Counter<u64>,
    pub messages_in_channel: Gauge<i64>,
    pub uptime: Gauge<f64>,
}

impl Metrics for WavsMetrics {
    fn init(meter: &Meter) -> WavsMetrics {
        WavsMetrics {
            total_messages_processed: meter
                .u64_counter("total_messages_processed")
                .with_description("Total number of messages processed")
                .build(),
            total_errors: meter
                .u64_counter("total_errors")
                .with_description("Total number of errors encountered")
                .build(),
            messages_in_channel: meter
                .i64_gauge("messages_in_channel")
                .with_description("Current number of messages in a channel")
                .build(),
            uptime: meter
                .f64_gauge("uptime_seconds")
                .with_description("System uptime in seconds")
                .build(),
        }
    }
}

impl WavsMetrics {
    pub fn add_processed_messages(&self, count: u64) {
        self.total_messages_processed.add(count, &[]);
        // or with attributes
        // self.total_messages_processed.add(count, &[KeyValue::new("source", "wav-decoder")]);
    }
}
