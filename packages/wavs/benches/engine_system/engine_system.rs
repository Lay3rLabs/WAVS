use criterion::Criterion;
use std::sync::Arc;
use tokio::sync::oneshot;
use wavs::submission_manager::chain_message::ChainMessage;
use wavs_benchmark_common::app_context::APP_CONTEXT;

use crate::setup::{SystemConfig, SystemSetup};

/// Main benchmark function for testing MultiEngineRunner throughput
///
/// This benchmark measures the performance of processing multiple concurrent
/// WASM component executions using the MultiEngineRunner. It tests the system's
/// ability to handle concurrent workloads across multiple threads.
pub fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi engine system");
    // Allow sufficient time for the benchmark to run multiple iterations with concurrent processing
    group.measurement_time(std::time::Duration::from_secs(180));
    // Use moderate sample size for consistent results
    group.sample_size(10);

    // Test different thread counts to see scaling behavior
    let thread_counts = vec![4, 16];
    let base_actions = 1000;

    for &thread_count in &thread_counts {
        let config = SystemConfig {
            n_actions: base_actions,
            thread_count,
        };

        group.bench_function(config.description(), move |b| {
            b.iter_with_setup(|| SystemSetup::new(config), run_simulation);
        });
    }

    group.finish();
}

/// Execute the configured number of concurrent engine actions through MultiEngineRunner
///
/// This function simulates a realistic system workload by:
/// 1. Setting up input/output channels for the MultiEngineRunner
/// 2. Starting the MultiEngineRunner in a background thread
/// 3. Sending multiple TriggerActions concurrently
/// 4. Collecting and validating all results
///
/// The benchmark measures end-to-end throughput including channel overhead,
/// thread coordination, and WASM execution time.
fn run_simulation(setup: Arc<SystemSetup>) {
    // This channel will signal when the simulation is finished
    let (finished_sender, finished_receiver) = oneshot::channel::<Vec<ChainMessage>>();
    let total_actions = setup.config.n_actions;

    // Collect all results
    let mut results_receiver = setup.result_receiver.lock().unwrap().take().unwrap();
    std::thread::spawn(move || {
        APP_CONTEXT.rt.block_on(async move {
            let mut received_results = Vec::new();
            while let Some(result) = results_receiver.recv().await {
                received_results.push(result);
                if received_results.len() == total_actions as usize {
                    // Notify that all results have been received
                    let _ = finished_sender.send(received_results);
                    break;
                }
            }
        });
    });

    let setup = APP_CONTEXT.rt.block_on(async move {
        let mut actions = setup.trigger_actions.lock().unwrap().take().unwrap();
        for (action, service) in actions.drain(..) {
            setup.action_sender.send((action, service)).await.unwrap();
        }
        setup
    });

    let received_results = APP_CONTEXT.rt.block_on(async {
        // Wait for all results to be received
        finished_receiver.await.unwrap()
    });

    // to keep the setup alive until the end of the simulation
    // we print out the thread count from setup.config
    println!(
        "Completed {} concurrent actions across {} threads",
        received_results.len(),
        setup.config.thread_count
    );
}
