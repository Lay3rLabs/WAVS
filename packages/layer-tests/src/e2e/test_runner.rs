// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::Result;
use futures::{
    stream::{self},
    StreamExt,
};
use std::sync::Arc;
use std::time::Instant;
use wavs_types::{EvmContractSubmission, Submit, Trigger};

use utils::context::AppContext;

use crate::{
    e2e::{clients::Clients, test_definition::TestDefinition, test_registry::TestRegistry},
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{SimpleEvmTriggerClient, TriggerId},
};

use super::helpers::wait_for_task_to_land;

/// Simplified test runner that leverages services directly attached to test definitions
pub struct TestRunner {
    ctx: AppContext,
    clients: Arc<Clients>,
    registry: Arc<TestRegistry>,
}

impl TestRunner {
    pub fn new(ctx: AppContext, clients: Clients, registry: TestRegistry) -> Self {
        Self {
            ctx,
            clients: Arc::new(clients),
            registry: Arc::new(registry),
        }
    }

    /// Run all tests in the registry
    pub fn run_tests(&self) -> Result<(), Vec<anyhow::Error>> {
        let tests = self.registry.list_all();
        tracing::info!("Running {} tests", tests.len());

        self.ctx.rt.block_on(async {
            let max_parallel = num_cpus::get();

            let stream = stream::iter(tests.into_iter().map(|test| {
                let clients = self.clients.clone();
                async move { self.execute_test(test, clients).await }
            }))
            .buffer_unordered(max_parallel);

            let mut failures = Vec::new();
            tokio::pin!(stream);

            while let Some(result) = stream.next().await {
                if let Err(err) = result {
                    failures.push(err);
                }
            }

            if !failures.is_empty() {
                tracing::error!("{} test(s) failed", failures.len());
                return Err(failures);
            }

            tracing::info!("All tests completed");
            Ok(())
        })
    }

    // Execute a single test with timings
    async fn execute_test(&self, test: &TestDefinition, clients: Arc<Clients>) -> Result<()> {
        let test_name = test.name.clone();
        let start_time = Instant::now();

        match run_test(test, &clients).await {
            Ok(_) => {
                let duration = start_time.elapsed();
                tracing::info!(
                    "Test {} passed (ran for {}ms)",
                    test_name,
                    duration.as_millis()
                );
                Ok(())
            }
            Err(e) => {
                let duration = start_time.elapsed();
                tracing::error!(
                    "Test {} failed after {}ms: {:?}",
                    test_name,
                    duration.as_millis(),
                    e
                );
                Err(e)
            }
        }
    }
}

/// Optimized implementation of running a single test
async fn run_test(test: &TestDefinition, clients: &Clients) -> Result<()> {
    // Get the service from the test
    let service = test.get_service()?;

    let submit_client = clients.get_evm_client(service.manager.chain_name());
    let submit_start_block = submit_client.provider.get_block_number().await?;

    // Run tasks sequentially according to test definition
    for (workflow_id, workflow) in service.workflows.iter() {
        let trigger_id = match &workflow.trigger {
            Trigger::EvmContractEvent {
                chain_name,
                address,
                event_hash: _,
            } => {
                let evm_client = clients.get_evm_client(chain_name);
                let client = SimpleEvmTriggerClient::new(evm_client, *address);

                client
                    .add_trigger(
                        test.workflows
                            .get(workflow_id)
                            .expect("Could not get workflow")
                            .input_data
                            .to_bytes()
                            .expect("Expected input for EVM contract trigger"),
                    )
                    .await?
            }
            Trigger::CosmosContractEvent {
                chain_name,
                address,
                event_type: _,
            } => {
                let client = SimpleCosmosTriggerClient::new(
                    clients.get_cosmos_client(chain_name).await,
                    address.clone(),
                );
                let trigger_id = client
                    .add_trigger(
                        test.workflows
                            .get(workflow_id)
                            .expect("Could not get workflow")
                            .input_data
                            .to_bytes()
                            .expect("Expected input for EVM contract trigger"),
                    )
                    .await?;

                TriggerId::new(trigger_id.u64())
            }
            Trigger::BlockInterval {
                chain_name: _,
                n_blocks: _,
                ..
            } => {
                // Hardcoded id since the current flow expects it to come from the event
                TriggerId::new(1337)
            }
            Trigger::Cron { .. } => TriggerId::new(1338),
            Trigger::Manual => unimplemented!(),
        };

        match &workflow.submit {
            Submit::EvmContract(EvmContractSubmission {
                chain_name,
                address,
                max_gas: _,
            }) => {
                wait_for_task_to_land(
                    clients.get_evm_client(chain_name),
                    *address,
                    trigger_id,
                    submit_start_block,
                )
                .await?;
            }
            Submit::Aggregator { .. } => {
                for aggregator in workflow.aggregators.iter() {
                    match aggregator {
                        wavs_types::Aggregator::Evm(EvmContractSubmission {
                            chain_name,
                            address,
                            ..
                        }) => {
                            wait_for_task_to_land(
                                clients.get_evm_client(chain_name),
                                *address,
                                trigger_id,
                                submit_start_block,
                            )
                            .await?;
                        }
                    }
                }
            }
            Submit::None => unimplemented!(),
        }
    }

    Ok(())
}
