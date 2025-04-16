use opentelemetry::metrics::{Counter, Gauge, Meter};
use opentelemetry::KeyValue;

#[derive(Clone)]
pub struct Metrics {
    pub total_messages_processed: Counter<u64>,
    pub total_errors: Counter<u64>,
    pub messages_in_channel: Gauge<i64>,
    pub registered_services: Gauge<i64>,
    pub uptime: Gauge<f64>,
}

impl Metrics {
    pub fn setup_metrics(meter: &Meter) -> Self {
        Metrics {
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
            registered_services: meter
                .i64_gauge("registered_services")
                .with_description("Number of services currently registered")
                .build(),
            uptime: meter
                .f64_gauge("uptime_seconds")
                .with_description("System uptime in seconds")
                .build(),
        }
    }

    pub fn update_messages_in_channel(&self, channel_id: &str, count: i64) {
        self.messages_in_channel
            .record(count, &[KeyValue::new("channel_id", channel_id.to_owned())]);
    }
}
