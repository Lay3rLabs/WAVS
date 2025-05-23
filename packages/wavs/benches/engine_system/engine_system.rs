use criterion::Criterion;
use wavs_benchmark_common::app_context::APP_CONTEXT;
use std::sync::Arc;
use std::time::Duration;

use crate::handle::{SystemHandle, SystemConfig};
use wavs::engine::runner::EngineRunner;

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
    let thread_counts = vec![1, 2, 4, 8];
    let base_actions = 1000;

    for &thread_count in &thread_counts {
        let config = SystemConfig {
            n_actions: base_actions,
            thread_count,
        };

        group.bench_function(config.description(), move |b| {
            b.iter_with_setup(|| SystemHandle::new(config), run_simulation);
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
fn run_simulation(handle: Arc<SystemHandle>) {
    APP_CONTEXT.rt.block_on(async move {
        let actions = handle.create_trigger_actions();
        let total_actions = actions.len();
        
        // Create channels for the MultiEngineRunner pipeline
        let (input_sender, input_receiver) = tokio::sync::mpsc::channel(total_actions);
        let (result_sender, mut result_receiver) = tokio::sync::mpsc::channel(total_actions);
        
        // Start the MultiEngineRunner
        handle.multi_runner.start(
            APP_CONTEXT.clone(), 
            input_receiver, 
            result_sender
        );

        // Send all actions to the runner
        for action in actions {
            input_sender.send(action).await.expect("Failed to send action");
        }
        
        // Close the input channel to signal completion
        drop(input_sender);
        
        // Collect all results
        let mut received_results = Vec::new();
        let mut timeout_count = 0;
        const MAX_TIMEOUTS: u32 = 10;
        
        while received_results.len() < total_actions {
            match tokio::time::timeout(Duration::from_millis(100), result_receiver.recv()).await {
                Ok(Some(result)) => {
                    received_results.push(result);
                    timeout_count = 0; // Reset timeout counter on successful receive
                }
                Ok(None) => {
                    // Channel closed, break if we have all results
                    break;
                }
                Err(_) => {
                    // Timeout occurred
                    timeout_count += 1;
                    if timeout_count >= MAX_TIMEOUTS {
                        panic!("Timeout waiting for results after {} attempts. Received {}/{} results", 
                               MAX_TIMEOUTS, received_results.len(), total_actions);
                    }
                }
            }
        }

        // Validate that we received all expected results
        assert_eq!(
            received_results.len(),
            total_actions,
            "Expected {} results, got {}",
            total_actions,
            received_results.len()
        );

        // Validate that all results contain the expected echo data
        for (i, result) in received_results.iter().enumerate() {
            let expected_data = format!("System benchmark action {}", i).into_bytes();
            assert_eq!(
                result.envelope.payload.to_vec(),
                expected_data,
                "Result {} payload mismatch",
                i
            );
        }

        println!(
            "Completed {} concurrent actions across {} threads", 
            total_actions, 
            handle.config.thread_count
        );
    });
}