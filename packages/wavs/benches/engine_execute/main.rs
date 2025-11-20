mod engine_execute;
mod setup;

use criterion::{criterion_group, criterion_main};
use setup::ExecuteSetup;
use std::sync::Arc;
use wavs_benchmark_common::app_context::APP_CONTEXT;

/// Execute the configured number of engine executions
///
/// This function creates a fresh InstanceDeps for each execution to ensure
/// isolated execution environments. Each execution uses a TriggerAction with
/// raw data to minimize overhead and focus on the engine execution performance.
pub fn run_simulation(setup: Arc<ExecuteSetup>) {
    APP_CONTEXT.rt.block_on(async move {
        let mut count = 0;
        let mut trigger_actions = setup.trigger_actions.lock().unwrap().take().unwrap();

        for (trigger_action, echo_data) in trigger_actions.drain(..) {
            // Create a new instance for this execution to ensure isolation
            let mut deps = setup.engine_setup.create_instance_deps(&trigger_action);

            // Execute the component and measure performance
            match wavs_engine::worlds::operator::execute::execute(&mut deps, trigger_action).await {
                Ok(responses) => {
                    if responses.is_empty() {
                        panic!("Execution returned no responses");
                    }
                    assert_eq!(responses[0].payload, echo_data, "Payload mismatch");
                }
                Err(err) => {
                    panic!("Execution failed: {err:?}");
                }
            }

            count += 1;
        }

        println!("Completed {count} engine executions");
    });
}

criterion_group!(benches, engine_execute::benchmark);
criterion_main!(benches);
