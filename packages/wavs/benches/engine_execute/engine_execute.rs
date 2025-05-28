use criterion::Criterion;

use crate::{
    run_simulation,
    setup::{ExecuteConfig, ExecuteSetup, SleepConfig},
};

/// Main benchmark function for testing Engine::execute() throughput
///
/// This benchmark measures the performance of executing a WASM component
/// using the echo_data.wasm component, which provides a minimal overhead
/// baseline for component execution performance.
pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine execute");
    // Use moderate sample size for consistent results
    group.sample_size(10);

    let config = ExecuteConfig {
        n_executions: 10_000,
        sleep_config: None,
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| ExecuteSetup::new(config.clone()), run_simulation);
    });

    let config = ExecuteConfig {
        n_executions: 1_000,
        sleep_config: Some(SleepConfig {
            sleep_ms: 5,
            sleep_kind: "sync".to_string(),
        }),
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| ExecuteSetup::new(config.clone()), run_simulation);
    });

    let config = ExecuteConfig {
        n_executions: 1_000,
        sleep_config: Some(SleepConfig {
            sleep_ms: 5,
            sleep_kind: "async".to_string(),
        }),
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| ExecuteSetup::new(config.clone()), run_simulation);
    });

    group.finish();
}
