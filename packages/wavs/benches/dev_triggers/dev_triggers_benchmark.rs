use criterion::Criterion;
use reqwest::Client;
use std::sync::Arc;

use crate::setup::{DevTriggersRuntime, DevTriggersSetup};

/// Benchmark for dev triggers POST endpoint performance
///
/// This benchmark tests the performance of the POST /dev/triggers endpoint
/// which is used for manually triggering workflows through the dev API.
/// It measures the throughput of sending trigger requests to the server.
pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("dev triggers");

    // Use smaller sample size for faster execution
    group.sample_size(10);

    // Use request count as the primary metric, payload stays constant
    let test_configs = vec![
        ("100_requests", 100usize),
        ("500_requests", 500usize),
        ("1000_requests", 1000usize),
    ];

    for (name, request_count) in test_configs {
        group.bench_function(name, move |b| {
            b.iter_with_setup(
                || {
                    let setup = DevTriggersSetup::new();
                    setup.start_runtime()
                },
                |runtime| run_dev_triggers_benchmark(runtime, request_count),
            );
        });
    }

    group.finish();
}

/// Run the dev triggers benchmark
///
/// This function creates a test server with real WASM execution,
/// sends HTTP POST requests to the /dev/triggers endpoint, and
/// executes all requests sequentially. The function blocks until
/// all requests are completed, allowing criterion to measure
/// the total execution time.
fn run_dev_triggers_benchmark(runtime: Arc<DevTriggersRuntime>, request_count: usize) {
    // Use the common app context for consistency with other benchmarks
    wavs_benchmark_common::app_context::APP_CONTEXT
        .rt
        .block_on(async move {
            // Create HTTP client
            let client = Client::new();

            runtime.submit_requests(&client, request_count).await;

            runtime.wait_for_messages(request_count).await;

            #[cfg(debug_assertions)]
            {
                runtime.wait_and_validate_packets(request_count).await;
            }
        });
}
