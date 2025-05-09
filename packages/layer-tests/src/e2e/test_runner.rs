// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::{Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use std::sync::Arc;
use std::time::Instant;

use utils::context::AppContext;

use crate::e2e::{
    add_task::{add_task, wait_for_task_to_land},
    clients::Clients,
    config::Configs,
    test_definition::TestDefinition,
    test_registry::TestRegistry,
};

/// Simplified test runner that leverages services directly attached to test definitions
pub struct TestRunner {
    ctx: AppContext,
    configs: Arc<Configs>,
    clients: Arc<Clients>,
    registry: Arc<TestRegistry>,
}

impl TestRunner {
    pub fn new(
        ctx: AppContext,
        configs: Configs,
        clients: Clients,
        registry: TestRegistry,
    ) -> Self {
        Self {
            ctx,
            configs: Arc::new(configs),
            clients: Arc::new(clients),
            registry: Arc::new(registry),
        }
    }

    /// Run all tests in the registry
    pub fn run_tests(&self) {
        // Collect tests that have services attached
        let tests: Vec<_> = self
            .registry
            .list_all()
            .into_iter()
            .filter(|test| test.has_service())
            .collect();

        if tests.is_empty() {
            tracing::warn!("No runnable tests found with attached services");
            return;
        }

        tracing::info!("Running {} tests", tests.len());

        let mut concurrent_futures = FuturesUnordered::new();

        for test in tests {
            let configs = self.configs.clone();
            let clients = self.clients.clone();
            let test_name = test.name.clone();

            let fut = async move {
                tracing::info!("Running test: {}", test_name);
                let start_time = Instant::now();

                match run_test(test, &configs, &clients).await {
                    Ok(_) => {
                        let duration = start_time.elapsed();
                        tracing::info!(
                            "Test {} passed (ran for {}ms)",
                            test_name,
                            duration.as_millis()
                        );
                    }
                    Err(e) => {
                        let duration = start_time.elapsed();
                        tracing::error!(
                            "Test {} failed after {}ms: {:?}",
                            test_name,
                            duration.as_millis(),
                            e
                        );
                    }
                }
            };

            concurrent_futures.push(fut);
        }

        self.ctx.rt.block_on(async move {
            tracing::info!("\n\n Running concurrent tests from TestDefinitions...");
            while (concurrent_futures.next().await).is_some() {}
            tracing::info!("All tests completed");
        });
    }

    /// Run a specific test by name
    pub fn run_test_by_name(&self, name: &str) -> Result<()> {
        let test = self
            .registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Test not found: {}", name))?;

        if !test.has_service() {
            return Err(anyhow::anyhow!("Test {} has no attached service", name));
        }

        let configs = self.configs.clone();
        let clients = self.clients.clone();

        self.ctx.rt.block_on(async move {
            tracing::info!("Running test: {}", name);
            let start_time = Instant::now();

            match run_test(test, &configs, &clients).await {
                Ok(_) => {
                    let duration = start_time.elapsed();
                    tracing::info!("Test {} passed (ran for {}ms)", name, duration.as_millis());

                    Ok(())
                }
                Err(e) => {
                    let duration = start_time.elapsed();
                    tracing::error!(
                        "Test {} failed after {}ms: {:?}",
                        name,
                        duration.as_millis(),
                        e
                    );

                    Err(e)
                }
            }
        })
    }
}

/// Run a single test
async fn run_test(test: &TestDefinition, configs: &Configs, clients: &Clients) -> Result<()> {
    // Get the service from the test
    let service = test.get_service();
    let service_id = service.id.to_string();

    // For this example, we'll assume we're testing the first workflow
    let (workflow_id, workflow) = service
        .workflows
        .iter()
        .next()
        .context("No workflows found in service")?;

    // Determine how many tasks to run based on test definition
    let n_tasks = test.num_tasks as usize;

    let submit_client = clients.get_evm_client(service.manager.chain_name());
    let submit_start_block = submit_client.provider.get_block_number().await?;

    // Run tasks according to test definition
    for task_number in 1..=n_tasks {
        let is_final = task_number == n_tasks;

        let input_data = test.input_data.to_bytes();

        let (trigger_id, signed_data) = add_task(
            clients,
            service_id.clone(),
            Some(workflow_id.to_string()),
            input_data,
            submit_client.clone(),
            submit_start_block,
            is_final,
        )
        .await?;

        if is_final {
            let signed_data = signed_data.context("no signed data returned")?;

            // Verify the output matches the expected output in the test definition
            if !test
                .expected_output
                .matches(&signed_data.data, &test.input_data)
            {
                return Err(anyhow::anyhow!(
                    "Output does not match expected output for test: {}",
                    test.name
                ));
            }

            // Handle multi-trigger service if specified
            if test.use_multi_trigger && test.multi_trigger_service.is_some() {
                let multi_trigger_service = test.multi_trigger_service.as_ref().unwrap();
                let multi_trigger_workflow =
                    multi_trigger_service.workflows.values().next().unwrap();

                // Determine the submission address
                let address = match &multi_trigger_workflow.submit {
                    wavs_types::Submit::None => {
                        return Err(anyhow::anyhow!("no submission in multi-trigger service"));
                    }
                    wavs_types::Submit::EvmContract(submit) => submit.address,
                    wavs_types::Submit::Aggregator { .. } => {
                        multi_trigger_service.manager.evm_address_unchecked()
                    }
                };

                // Wait for the second task to land
                let signed_data = wait_for_task_to_land(
                    submit_client.clone(),
                    address,
                    trigger_id,
                    submit_start_block,
                )
                .await;

                // Verify the second task's output
                if !test
                    .expected_output
                    .matches(&signed_data.data, &test.input_data)
                {
                    return Err(anyhow::anyhow!(
                        "Multi-trigger output does not match expected output for test: {}",
                        test.name
                    ));
                }
            }
        }
    }

    Ok(())
}
