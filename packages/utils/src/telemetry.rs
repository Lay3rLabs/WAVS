use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
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

pub fn setup_metrics(collector: &str, service_name: &str) -> SdkMeterProvider {
    let endpoint = format!("{}/api/v1/otlp/v1/metrics", collector);

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint)
        .build()
        .expect("Failed to build OTLP exporter!");

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .with_periodic_exporter(exporter)
        .build();

    global::set_meter_provider(meter_provider.clone());

    tracing::info!("Metrics enabled and exporting to {}", collector);

    meter_provider
}

use opentelemetry::metrics::{Counter, Gauge, Meter, UpDownCounter};

pub struct Metrics {
    pub http: HttpMetrics,
    pub wavs: WavsMetrics,
}

impl Metrics {
    pub fn init(meter: &Meter) -> Self {
        Self {
            http: HttpMetrics::init(meter),
            wavs: WavsMetrics::init(meter),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HttpMetrics {
    pub registered_services: UpDownCounter<i64>,
}

impl HttpMetrics {
    pub const LABEL: &'static str = "http";

    pub fn init(meter: &Meter) -> Self {
        HttpMetrics {
            registered_services: meter
                .i64_up_down_counter(format!("{}_registered_services", Self::LABEL))
                .with_description("Number of services currently registered")
                .build(),
        }
    }

    pub fn increment_registered_services(&self) {
        self.registered_services.add(1, &[]);
    }

    pub fn decrement_registered_services(&self) {
        self.registered_services.add(-1, &[]);
    }
}

#[derive(Clone, Debug)]
pub struct WavsMetrics {
    pub engine: EngineMetrics,
    pub dispatcher: DispatcherMetrics,
    pub submission: SubmissionMetrics,
    pub trigger: TriggerMetrics,
}

impl WavsMetrics {
    pub fn init(meter: &Meter) -> Self {
        Self {
            engine: EngineMetrics::init(meter),
            dispatcher: DispatcherMetrics::init(meter),
            submission: SubmissionMetrics::init(meter),
            trigger: TriggerMetrics::init(meter),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EngineMetrics {
    pub total_threads: Counter<u64>,
    pub total_errors: Counter<u64>,
}

impl EngineMetrics {
    pub const LABEL: &'static str = "engine";

    pub fn init(meter: &Meter) -> Self {
        Self {
            total_threads: meter
                .u64_counter(format!("{}_total_threads", Self::LABEL))
                .with_description("Total number of threads being used currently")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}_total_errors", Self::LABEL))
                .with_description("Total number of errors encountered")
                .build(),
        }
    }

    pub fn increment_total_errors(&self) {
        self.total_errors.add(1, &[]);
    }
}

#[derive(Clone, Debug)]
pub struct DispatcherMetrics {
    pub messages_in_channel: Gauge<u64>,
    pub total_errors: Counter<u64>,
}

impl DispatcherMetrics {
    pub const LABEL: &'static str = "dispatcher";

    pub fn init(meter: &Meter) -> Self {
        Self {
            messages_in_channel: meter
                .u64_gauge(format!("{}_messages_in_channel", Self::LABEL))
                .with_description("Current number of messages in a channel")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}_total_errors", Self::LABEL))
                .with_description("Total number of errors encountered")
                .build(),
        }
    }

    pub fn increment_total_errors(&self) {
        self.total_errors.add(1, &[]);
    }
}

#[derive(Clone, Debug)]
pub struct SubmissionMetrics {
    pub total_messages_processed: Counter<u64>,
    pub total_errors: Counter<u64>,
}

impl SubmissionMetrics {
    pub const LABEL: &'static str = "submission";

    pub fn init(meter: &Meter) -> Self {
        Self {
            total_messages_processed: meter
                .u64_counter(format!("{}_total_messages_processed", Self::LABEL))
                .with_description("Total number of messages processed")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}_total_errors", Self::LABEL))
                .with_description("Total number of errors encountered")
                .build(),
        }
    }

    pub fn increment_total_processed_messages(&self, source: &str) {
        self.total_messages_processed
            .add(1, &[KeyValue::new("source", source.to_owned())]);
    }

    pub fn increment_total_errors(&self) {
        self.total_errors.add(1, &[]);
    }
}

#[derive(Clone, Debug)]
pub struct TriggerMetrics {
    pub total_errors: Counter<u64>,
}

impl TriggerMetrics {
    pub const LABEL: &'static str = "trigger";

    pub fn init(meter: &Meter) -> Self {
        Self {
            total_errors: meter
                .u64_counter(format!("{}_total_errors", Self::LABEL))
                .with_description("Total number of errors encountered")
                .build(),
        }
    }

    pub fn increment_total_errors(&self) {
        self.total_errors.add(1, &[]);
    }
}
