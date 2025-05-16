use anyhow::{Context, Result};
use criterion::*;
use futures::StreamExt;
use opentelemetry::global;
use std::collections::BTreeMap;
use std::num::NonZero;
use std::process::{Child, Command};
use tokio_stream::wrappers::ReceiverStream;
use utils::config::{ChainConfigs, EvmChainConfig};
use utils::context::AppContext;
use utils::telemetry::TriggerMetrics;
use wavs::config::Config;
use wavs::{
    apis::trigger::{TriggerError, TriggerManager},
    triggers::core::CoreTriggerManager,
};
use wavs_types::{ChainName, ServiceID, Trigger, TriggerConfig, WorkflowID};

/// Benchmark the TriggerManager's handling of block interval triggers
pub fn benchmark_trigger_system(c: &mut Criterion) {
    // Initialize variables
    let mut anvil_processes = Vec::new();
    let mut evm_chain_configs = BTreeMap::new();
    let base_port = 8545;
    let num_chains = 10;
    let triggers_per_chain = 1000;
    let action_limit = 10000;

    // Spawn Anvil instances and create configs simultaneously
    println!("Starting Anvil instances...");
    let chain_names: Vec<ChainName> = (0..num_chains)
        .map(|i| {
            let chain_name: ChainName = ChainName::new(format!("test-chain-{}", i)).unwrap();
            let port = base_port + i;
            let chain_id = 1337 + i;

            // Spawn Anvil instance with auto-mining every 1 second
            let anvil = spawn_anvil(port, chain_id)
                .unwrap_or_else(|_| panic!("Failed to spawn Anvil instance {}", i));
            anvil_processes.push(anvil);

            // Create config for this chain
            let evm_config = EvmChainConfig {
                chain_id: chain_id.to_string(),
                ws_endpoint: Some(format!("ws://localhost:{}", port)),
                http_endpoint: Some(format!("http://localhost:{}", port)),
                aggregator_endpoint: None,
                faucet_endpoint: None,
                poll_interval_ms: Some(100), // Fast polling for benchmarks
            };

            // Add to EVM chain configs
            evm_chain_configs.insert(chain_name.clone(), evm_config);

            chain_name
        })
        .collect();

    // Create the config
    let config = Config {
        chains: ChainConfigs {
            evm: evm_chain_configs,
            cosmos: BTreeMap::new(),
        },
        active_trigger_chains: chain_names.clone(),
        ..Config::default()
    };

    // Create metrics
    let metrics = TriggerMetrics::new(&global::meter("wavs-benchmark"));

    let mut group = c.benchmark_group("trigger_processing_benchmarks");
    group.sample_size(10);

    // Create a separate app context explicitly for this benchmark
    let app_context = AppContext::new();

    println!("Setting up {} triggers per chain...", triggers_per_chain);
    group.bench_function("block_interval_processing", |b| {
        b.to_async(&*app_context.rt).iter_batched(
            // Setup - create a fresh trigger manager for each iteration
            || {
                // Create a fresh trigger manager
                let trigger_manager = CoreTriggerManager::new(&config, metrics.clone())
                    .expect("Failed to create CoreTriggerManager");

                // Setup triggers asynchronously
                setup_block_interval_triggers(
                    &trigger_manager,
                    chain_names.as_slice(),
                    triggers_per_chain,
                )
                .expect("Failed to setup triggers");

                // Return what we need for the benchmark
                trigger_manager.start(app_context.clone()).unwrap()
            },
            // Async benchmark function
            |receiver| async move {
                // Create a stream from the receiver
                let mut action_stream = ReceiverStream::new(receiver);

                // Count processed trigger actions
                let mut action_count = 0;
                let start_time = std::time::Instant::now();

                // Run for a fixed # of actions
                while let Some(action) = action_stream.next().await {
                    black_box(action);
                    action_count += 1;

                    if action_count >= action_limit {
                        break;
                    }
                }

                let elapsed = start_time.elapsed();
                let rate = action_count as f64 / elapsed.as_secs_f64();
                println!(
                    "Processed {} trigger actions in {:.2}s ({:.2} actions/sec)",
                    action_count,
                    elapsed.as_secs_f64(),
                    rate
                );

                // Return the count for the benchmark
                action_count
            },
            BatchSize::PerIteration,
        )
    });

    group.finish();

    // Shutdown after all benchmarks complete
    app_context.kill();

    // Clean up all Anvil instances
    println!("Cleaning up Anvil instances...");
    for (i, mut process) in anvil_processes.into_iter().enumerate() {
        if let Err(e) = process.kill() {
            eprintln!("Failed to kill Anvil instance {}: {}", i, e);
        }
    }
}

/// Spawn an Anvil instance with specific port and chain ID
fn spawn_anvil(port: usize, chain_id: usize) -> Result<Child> {
    Command::new("anvil")
        .args([
            "--port",
            &port.to_string(),
            "--chain-id",
            &chain_id.to_string(),
            "--block-time",
            "1", // Mine a block every second
            "--quiet",
            "--disable-console-log",
        ])
        .stdout(std::process::Stdio::null()) // Redirect stdout to null
        .spawn()
        .context("Spawn Anvil")
}

/// Set up block interval triggers for each chain with NO start_block
fn setup_block_interval_triggers(
    trigger_manager: &CoreTriggerManager,
    chain_names: &[ChainName],
    triggers_per_chain: usize,
) -> Result<(), TriggerError> {
    for (chain_idx, chain_name) in chain_names.iter().enumerate() {
        // Create triggers with varying intervals
        for trigger_idx in 0..triggers_per_chain {
            let n_blocks = NonZero::new((trigger_idx % 10 + 1) as u32).unwrap();

            let trigger_config = TriggerConfig {
                trigger: Trigger::BlockInterval {
                    chain_name: chain_name.clone(),
                    n_blocks,
                    start_block: None, // No start block - activate immediately
                    end_block: None,
                },
                service_id: ServiceID::new(format!("service-{}", chain_idx)).unwrap(),
                workflow_id: WorkflowID::new(format!("workflow-{}-{}", chain_idx, trigger_idx))
                    .unwrap(),
            };

            trigger_manager.add_trigger(trigger_config)?;
        }
    }

    Ok(())
}

criterion_group!(benches, benchmark_trigger_system);
criterion_main!(benches);
