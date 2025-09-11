use criterion::Criterion;
use reqwest::Client;
use std::time::{Duration, Instant};

use crate::setup::DevTriggersRuntime;

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
            b.iter_custom(|iters| {
                let mut total = Duration::from_secs(0);
                for _ in 0..iters {
                    let runtime = DevTriggersRuntime::new();
                    let start = Instant::now();
                    let client = Client::new();
                    wavs_benchmark_common::app_context::APP_CONTEXT
                        .rt
                        .block_on(async {
                            runtime.submit_requests(&client, request_count).await;
                            runtime.wait_for_messages(request_count).await;

                            #[cfg(debug_assertions)]
                            {
                                runtime.wait_and_validate_packets(request_count).await;
                            }
                        });
                    total += start.elapsed();

                    // Ensure shutdown happens outside measured time
                    drop(runtime);
                }
                total
            })
        });
    }

    group.finish();
}
