use criterion::Criterion;
use reqwest::Client;
use std::time::{Duration, Instant};

use crate::setup::{ComponentConfig, DevTriggersRuntime};

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
    let component_configs = [
        ComponentConfig::Default,
        ComponentConfig::HotLoop { sleep_ms: 10 },
        ComponentConfig::HotLoop { sleep_ms: 100 },
    ];
    let test_configs = [
        ("100_requests", 100usize),
        ("500_requests", 500usize),
        ("1000_requests", 1000usize),
    ];

    for component_config in component_configs.iter() {
        for (name, request_count) in test_configs.iter() {
            let benchmark_name = match component_config {
                ComponentConfig::Default => format!("{name}_default"),
                ComponentConfig::HotLoop { sleep_ms: ms } => {
                    format!("{name}_hotloop_{ms}ms")
                }
            };
            group.bench_function(&benchmark_name, move |b| {
                b.iter_custom(|iters| {
                    let mut total = Duration::from_secs(0);
                    for _ in 0..iters {
                        let runtime = DevTriggersRuntime::new(component_config.clone());
                        let start = Instant::now();
                        let client = Client::new();
                        wavs_benchmark_common::app_context::APP_CONTEXT
                            .rt
                            .block_on(async {
                                runtime
                                    .submit_requests(&client, *request_count, false)
                                    .await;
                                runtime.wait_for_messages(*request_count).await;
                                runtime.wait_and_validate_packets(*request_count).await;
                            });
                        total += start.elapsed();

                        // Ensure shutdown happens outside measured time
                        drop(runtime);
                    }
                    total
                })
            });
        }
    }

    group.finish();
}
