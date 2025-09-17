use std::time::Duration;

use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter, UpDownCounter};
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::metrics::PeriodicReader;
use opentelemetry_sdk::{
    metrics::SdkMeterProvider,
    resource::Resource,
    trace::{self, Sampler, SdkTracerProvider},
};
use tracing_subscriber::layer::SubscriberExt;
use wavs_types::ChainKey;

const DEFAULT_PROMETHEUS_PUSH_INTERVAL: u64 = 30; // seconds

pub fn setup_tracing(
    collector: &str,
    service_name: &str,
    filters: tracing_subscriber::EnvFilter,
) -> SdkTracerProvider {
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
        .with_resource(
            Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .build();
    global::set_tracer_provider(provider.clone());
    let tracer = provider.tracer(format!("{service_name}-tracer"));
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

pub fn setup_metrics(
    collector: &str,
    service_name: &str,
    push_interval_secs: Option<u64>,
) -> SdkMeterProvider {
    let endpoint = format!("{collector}/api/v1/otlp/v1/metrics");

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint)
        .build()
        .expect("Failed to build OTLP exporter!");

    let reader = PeriodicReader::builder(exporter)
        .with_interval(Duration::from_secs(
            push_interval_secs.unwrap_or(DEFAULT_PROMETHEUS_PUSH_INTERVAL),
        ))
        .build();

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(
            Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .with_reader(reader)
        .build();

    global::set_meter_provider(meter_provider.clone());

    tracing::info!("Metrics enabled and exporting to {}", collector);

    meter_provider
}

pub struct Metrics {
    pub http: HttpMetrics,
    pub wavs: WavsMetrics,
}

impl Metrics {
    pub fn new(meter: Meter) -> Self {
        Self {
            http: HttpMetrics::new(meter.clone()),
            wavs: WavsMetrics::new(meter),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AggregatorMetrics {
    pub packets_received: Counter<u64>,
    pub packets_processed: Counter<u64>,
    pub packets_failed: Counter<u64>,
    pub processing_latency: Histogram<f64>,
    pub total_errors: Counter<u64>,
    pub engine_executions_success: Counter<u64>,
    pub engine_executions_failed: Counter<u64>,
    pub engine_execution_duration: Histogram<f64>,
    pub engine_fuel_consumption: Histogram<u64>,
}

impl AggregatorMetrics {
    pub const NAMESPACE: &'static str = "aggregator";

    pub fn new(meter: Meter) -> Self {
        Self {
            packets_received: meter
                .u64_counter(format!("{}.packets_received", Self::NAMESPACE))
                .with_description("Total packets received by aggregator")
                .build(),
            packets_processed: meter
                .u64_counter(format!("{}.packets_processed", Self::NAMESPACE))
                .with_description("Total packets successfully processed")
                .build(),
            packets_failed: meter
                .u64_counter(format!("{}.packets_failed", Self::NAMESPACE))
                .with_description("Total packets that failed processing")
                .build(),
            processing_latency: meter
                .f64_histogram(format!("{}.processing_latency_seconds", Self::NAMESPACE))
                .with_description("Packet processing latency in seconds")
                .with_boundaries(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0])
                .build(),
            total_errors: meter
                .u64_counter(format!("{}.total_errors", Self::NAMESPACE))
                .with_description("Total errors in aggregator")
                .build(),
            engine_executions_success: meter
                .u64_counter(format!("{}.engine_executions_success", Self::NAMESPACE))
                .with_description("Successful engine executions in aggregator")
                .build(),
            engine_executions_failed: meter
                .u64_counter(format!("{}.engine_executions_failed", Self::NAMESPACE))
                .with_description("Failed engine executions in aggregator")
                .build(),
            engine_execution_duration: meter
                .f64_histogram(format!("{}.engine_execution_seconds", Self::NAMESPACE))
                .with_description("Engine execution duration in seconds")
                .with_boundaries(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0])
                .build(),
            engine_fuel_consumption: meter
                .u64_histogram(format!("{}.engine_fuel_consumption", Self::NAMESPACE))
                .with_description("Fuel consumed per engine execution")
                .with_boundaries(vec![
                    1000.0,
                    10000.0,
                    100000.0,
                    1000000.0,
                    10000000.0,
                    100000000.0,
                ])
                .build(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HttpMetrics {
    pub registered_services: UpDownCounter<i64>,
    pub meter: Meter,
}

impl HttpMetrics {
    pub const NAMESPACE: &'static str = "http";

    pub fn new(meter: Meter) -> Self {
        Self {
            registered_services: meter
                .i64_up_down_counter(format!("{}.registered_services", Self::NAMESPACE))
                .with_description("Number of services currently registered")
                .build(),
            meter,
        }
    }

    pub fn record_trigger_simulation_completed(&self, duration: f64, trigger_count: usize) {
        // first, histogram
        let buckets = match trigger_count {
            1 => vec![0.01, 0.025, 0.05, 0.075, 0.1, 0.15, 0.2, 0.3, 0.5],
            2..=100 => vec![0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.6, 1.0],
            101..=1000 => vec![0.5, 0.8, 1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 5.0],
            1001..=10000 => vec![5.0, 8.0, 10.0, 12.0, 15.0, 18.0, 25.0, 30.0, 40.0],
            _ => vec![20.0, 30.0, 40.0, 50.0, 60.0, 80.0, 100.0, 120.0, 180.0],
        };

        self.meter
            .f64_histogram(format!(
                "{}.simulated_{}_trigger_seconds",
                Self::NAMESPACE,
                trigger_count
            ))
            .with_description("Duration to process simulated triggers")
            .with_boundaries(buckets)
            .build()
            .record(
                duration,
                &[KeyValue::new("batch_size", trigger_count as i64)],
            );

        // Also record as a gauge for "latest value" queries
        self.meter
            .f64_gauge(format!(
                "{}.latest_simulated_{}_trigger_seconds",
                Self::NAMESPACE,
                trigger_count
            ))
            .with_description("Most recent duration for simulated triggers")
            .build()
            .record(
                duration,
                &[KeyValue::new("batch_size", trigger_count as i64)],
            );
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
    pub fn new(meter: Meter) -> Self {
        Self {
            engine: EngineMetrics::new(meter.clone()),
            dispatcher: DispatcherMetrics::new(meter.clone()),
            submission: SubmissionMetrics::new(meter.clone()),
            trigger: TriggerMetrics::new(meter),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EngineMetrics {
    pub total_threads: Counter<u64>,
    pub total_errors: Counter<u64>,
    pub execution_duration: Histogram<f64>,
    pub fuel_consumption: Histogram<u64>,
    pub executions_success: Counter<u64>,
    pub executions_failed: Counter<u64>,
}

impl EngineMetrics {
    pub const NAMESPACE: &'static str = "engine";

    pub fn new(meter: Meter) -> Self {
        Self {
            total_threads: meter
                .u64_counter(format!("{}.total_threads", Self::NAMESPACE))
                .with_description("Total number of threads being used currently")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}.total_errors", Self::NAMESPACE))
                .with_description("Total number of errors encountered")
                .build(),
            execution_duration: meter
                .f64_histogram(format!("{}.execution_seconds", Self::NAMESPACE))
                .with_description("WASM execution duration in seconds")
                .with_boundaries(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0])
                .build(),
            fuel_consumption: meter
                .u64_histogram(format!("{}.fuel_consumption", Self::NAMESPACE))
                .with_description("Fuel consumed per WASM execution")
                .with_boundaries(vec![
                    1000.0,
                    10000.0,
                    100000.0,
                    1000000.0,
                    10000000.0,
                    100000000.0,
                ])
                .build(),
            executions_success: meter
                .u64_counter(format!("{}.executions_success", Self::NAMESPACE))
                .with_description("Successful WASM executions")
                .build(),
            executions_failed: meter
                .u64_counter(format!("{}.executions_failed", Self::NAMESPACE))
                .with_description("Failed WASM executions")
                .build(),
        }
    }

    pub fn increment_total_errors(&self, error: &str) {
        self.total_errors
            .add(1, &[KeyValue::new("error", error.to_owned())]);
    }

    pub fn record_execution(
        &self,
        duration: f64,
        fuel: u64,
        service_id: &str,
        workflow_id: &str,
        success: bool,
    ) {
        let labels = &[
            KeyValue::new("service_id", service_id.to_owned()),
            KeyValue::new("workflow_id", workflow_id.to_owned()),
        ];

        self.execution_duration.record(duration, labels);
        self.fuel_consumption.record(fuel, labels);

        if success {
            self.executions_success.add(1, labels);
        } else {
            self.executions_failed.add(1, labels);
        }
    }
}

#[derive(Clone, Debug)]
pub struct DispatcherMetrics {
    pub messages_in_channel: Gauge<u64>,
    pub total_errors: Counter<u64>,
    pub channel_closed_errors: Counter<u64>, // Tracks when send fails because receiver is dropped
}

impl DispatcherMetrics {
    pub const NAMESPACE: &'static str = "dispatcher";

    pub fn new(meter: Meter) -> Self {
        Self {
            messages_in_channel: meter
                .u64_gauge(format!("{}.messages_in_channel", Self::NAMESPACE))
                .with_description("Current number of messages in a channel")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}.total_errors", Self::NAMESPACE))
                .with_description("Total number of errors encountered")
                .build(),
            channel_closed_errors: meter
                .u64_counter(format!("{}.channel_closed_errors", Self::NAMESPACE))
                .with_description("Send failures due to receiver being dropped")
                .build(),
        }
    }

    pub fn increment_total_errors(&self, error: &str) {
        self.total_errors
            .add(1, &[KeyValue::new("error", error.to_owned())]);
    }
}

impl Default for DispatcherMetrics {
    fn default() -> Self {
        Self::new(global::meter("wavs_metrics"))
    }
}

#[derive(Clone, Debug)]
pub struct SubmissionMetrics {
    pub total_messages_processed: Counter<u64>,
    pub total_errors: Counter<u64>,
    pub submission_latency: Histogram<f64>, // Time from WASM completion to chain submission
    pub submissions_success: Counter<u64>,
    pub submissions_failed: Counter<u64>,
}

impl SubmissionMetrics {
    pub const NAMESPACE: &'static str = "submission";

    pub fn new(meter: Meter) -> Self {
        Self {
            total_messages_processed: meter
                .u64_counter(format!("{}.total_messages_processed", Self::NAMESPACE))
                .with_description("Total number of messages processed")
                .build(),
            total_errors: meter
                .u64_counter(format!("{}.total_errors", Self::NAMESPACE))
                .with_description("Total number of errors encountered")
                .build(),
            submission_latency: meter
                .f64_histogram(format!("{}.submission_latency_seconds", Self::NAMESPACE))
                .with_description("Time from WASM completion to chain submission")
                .with_boundaries(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0])
                .build(),
            submissions_success: meter
                .u64_counter(format!("{}.submissions_success", Self::NAMESPACE))
                .with_description("Successful chain submissions")
                .build(),
            submissions_failed: meter
                .u64_counter(format!("{}.submissions_failed", Self::NAMESPACE))
                .with_description("Failed chain submissions")
                .build(),
        }
    }

    pub fn increment_total_processed_messages(&self, source: &str) {
        self.total_messages_processed
            .add(1, &[KeyValue::new("source", source.to_owned())]);
    }

    pub fn increment_total_errors(&self, error: &str) {
        self.total_errors
            .add(1, &[KeyValue::new("error", error.to_owned())]);
    }

    pub fn record_submission(&self, latency: f64, chain: &str, success: bool) {
        let labels = &[KeyValue::new("chain", chain.to_owned())];

        self.submission_latency.record(latency, labels);

        if success {
            self.submissions_success.add(1, labels);
        } else {
            self.submissions_failed.add(1, labels);
        }
    }
}

#[derive(Clone, Debug)]
pub struct TriggerMetrics {
    pub total_errors: Counter<u64>,
    pub triggers_fired: Counter<u64>,
    pub sent_dispatcher_command_latency: Histogram<f64>,
}

impl TriggerMetrics {
    pub const NAMESPACE: &'static str = "trigger";

    pub fn new(meter: Meter) -> Self {
        Self {
            total_errors: meter
                .u64_counter(format!("{}.total_errors", Self::NAMESPACE))
                .with_description("Total number of errors encountered")
                .build(),
            triggers_fired: meter
                .u64_counter(format!("{}.triggers_fired", Self::NAMESPACE))
                .with_description("Total triggers fired")
                .build(),
            sent_dispatcher_command_latency: meter
                .f64_histogram(format!(
                    "{}.sent_dispatcher_command_latency_seconds",
                    Self::NAMESPACE
                ))
                .with_description("Time taken to send command to dispatcher")
                .with_boundaries(vec![0.001, 0.01, 0.05, 0.1, 0.2, 0.5, 1.0])
                .build(),
        }
    }

    pub fn increment_total_errors(&self, error: &str) {
        self.total_errors
            .add(1, &[KeyValue::new("error", error.to_owned())]);
    }

    pub fn record_trigger_fired(&self, chain: Option<&ChainKey>, trigger_type: &str) {
        self.triggers_fired.add(
            1,
            &[
                KeyValue::new("chain", chain.map(|c| c.to_string()).unwrap_or_default()),
                KeyValue::new("type", trigger_type.to_owned()),
            ],
        );
    }

    pub fn record_trigger_sent_dispatcher_command(&self, duration: f64) {
        self.sent_dispatcher_command_latency.record(duration, &[]);
    }
}
