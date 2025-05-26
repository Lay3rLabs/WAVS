use criterion::Criterion;
use std::sync::Arc;

use wavs_benchmark_common::{
    app_context::APP_CONTEXT,
    engine_execute_setup::{EngineSetup, EngineSetupConfig},
};

/// Main benchmark function for testing Engine::execute() throughput
///
/// This benchmark measures the performance of executing a WASM component
/// using the echo_raw.wasm component, which provides a minimal overhead
/// baseline for component execution performance.
pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine execute");
    // Allow sufficient time for the benchmark to run multiple iterations
    group.measurement_time(std::time::Duration::from_secs(120));
    // Use moderate sample size for consistent results
    group.sample_size(10);

    let config = EngineSetupConfig {
        n_executions: 10_000,
    };

    group.bench_function(config.description(), move |b| {
        b.iter_with_setup(|| EngineSetup::new(config), run_simulation);
    });

    group.finish();
}

/// Execute the configured number of engine executions
///
/// This function creates a fresh InstanceDeps for each execution to ensure
/// isolated execution environments. Each execution uses a TriggerAction with
/// raw data to minimize overhead and focus on the engine execution performance.
fn run_simulation(setup: Arc<EngineSetup>) {
    APP_CONTEXT.rt.block_on(async move {
        for execution_count in 1..=setup.config.n_executions {
            // Create a new instance for this execution to ensure isolation
            let mut deps = setup.create_instance_deps();

            // Create trigger action with raw test data
            let echo_data = format!("Execution number {}", execution_count).into_bytes();
            let trigger_action = setup.create_trigger_action(echo_data.clone());

            // Execute the component and measure performance
            match wavs_engine::execute(&mut deps, trigger_action).await {
                Ok(response) => {
                    let payload = response
                        .expect("Execution failed to generate a response")
                        .payload;
                    assert_eq!(payload, echo_data, "Payload mismatch");
                }
                Err(err) => {
                    panic!("Execution failed: {:?}", err);
                }
            }
        }

        println!("Completed {} engine executions", setup.config.n_executions);
    });
}
