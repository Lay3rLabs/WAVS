use criterion::Criterion;

use crate::{
    run_simulation,
    setup::{AsyncConfig, ExecuteConfig, ExecuteSetup},
};

/// Main benchmark function for testing Engine::execute() throughput
///
/// This benchmark measures the performance of executing a WASM component
/// using the echo_raw.wasm component with async configuration
pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine execute async");
    // Use moderate sample size for consistent results
    group.sample_size(10);

    let config = ExecuteConfig {
        n_executions: 1_000,
        async_config: Some(AsyncConfig::default()),
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| ExecuteSetup::new(config.clone()), run_simulation);
    });

    group.finish();
}
