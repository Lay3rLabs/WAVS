// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::{Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use utils::context::AppContext;
use utils::evm_client::EvmSigningClient;

use crate::{
    e2e::{
        add_task::{add_task, wait_for_task_to_land},
        clients::Clients,
        test_definition::TestDefinition,
        test_registry::TestRegistry,
    },
    example_evm_client::{example_submit::ISimpleSubmit::SignedData, TriggerId},
};

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

        // Group tests by chain to minimize chain switching
        let mut chain_grouped_tests: HashMap<String, Vec<&TestDefinition>> = HashMap::new();
        for test in tests {
            let chain_name = test.get_service().manager.chain_name().to_string();
            chain_grouped_tests
                .entry(chain_name)
                .or_default()
                .push(test);
        }

        let total_tests = chain_grouped_tests.values().map(|v| v.len()).sum::<usize>();
        tracing::info!(
            "Running {} tests across {} chains",
            total_tests,
            chain_grouped_tests.len()
        );

        // Use a buffer to control maximum concurrency
        let concurrency_limit = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);

        let mut concurrent_futures = FuturesUnordered::new();

        // Prepare a flattened list of tests, prioritizing by chain for better locality
        let mut remaining_tests = VecDeque::new();
        for (_, chain_tests) in chain_grouped_tests {
            remaining_tests.extend(chain_tests);
        }

        self.ctx.rt.block_on(async move {
            tracing::info!(
                "\n\n Running concurrent tests (max: {}) from TestDefinitions...",
                concurrency_limit
            );

            // Initial batch of tests
            for test in
                remaining_tests.drain(..std::cmp::min(concurrency_limit, remaining_tests.len()))
            {
                concurrent_futures.push(self.execute_test(test, self.clients.clone()));
            }

            // Process results and add more tests as capacity allows
            while let Some(_result) = concurrent_futures.next().await {
                // Add another test to the queue if available
                if !remaining_tests.is_empty() {
                    let next_test = remaining_tests.remove(0);
                    if let Some(test) = next_test {
                        tracing::info!("Running test: {}", test.name);
                        concurrent_futures.push(self.execute_test(test, self.clients.clone()));
                    }
                }
            }

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

        let clients = self.clients.clone();

        self.ctx.rt.block_on(async move {
            let start_time = Instant::now();

            let result = self.execute_test(test, clients).await;
            match result {
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

    // Execute a single test with timings
    async fn execute_test(&self, test: &TestDefinition, clients: Arc<Clients>) -> Result<()> {
        let test_name = test.name.clone();
        tracing::info!("Running test: {}", test_name);
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
    let service = test.get_service();
    let service_id = service.id.to_string();

    // For this example, we'll assume we're testing the first workflow
    let (workflow_id, _workflow) = service
        .workflows
        .iter()
        .next()
        .context("No workflows found in service")?;

    let submit_client = clients.get_evm_client(service.manager.chain_name());
    let submit_start_block = submit_client.provider.get_block_number().await?;

    // For multi-task tests that don't require sequential execution, run them concurrently
    if !test.use_multi_trigger {
        // Run tasks concurrently for faster execution
        let mut task_futures = FuturesUnordered::new();

        let input_data = test.input_data.to_bytes();
        let service_id_clone = service_id.clone();
        let workflow_id_clone = workflow_id.to_string();
        let submit_client_clone = submit_client.clone();
        let clients_clone = clients.clone();

        task_futures.push(async move {
            add_task(
                &clients_clone,
                service_id_clone,
                Some(workflow_id_clone),
                input_data,
                submit_client_clone,
                submit_start_block,
                true,
            )
            .await
        });

        // Process results
        let mut final_result: Option<(TriggerId, SignedData)> = None;
        while let Some(result) = task_futures.next().await {
            match result {
                Ok((trigger_id, Some(signed_data))) => {
                    final_result = Some((trigger_id, signed_data));
                }
                Ok(_) => {} // Non-final task, no validation needed
                Err(e) => return Err(e),
            }
        }

        // Validate the final result
        if let Some((trigger_id, signed_data)) = final_result {
            // Verify output
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
                handle_multi_trigger(test, &submit_client, trigger_id, submit_start_block).await?;
            }
        } else {
            return Err(anyhow::anyhow!(
                "No final result received for test: {}",
                test.name
            ));
        }
    } else {
        // For tests that require sequential execution
        // Run tasks sequentially according to test definition
        for task_number in 0..test.components.len() {
            let is_final = task_number == test.components.len() - 1;
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

                // Verify the output
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
                if test.multi_trigger_service.is_some() {
                    handle_multi_trigger(test, &submit_client, trigger_id, submit_start_block)
                        .await?;
                }
            }
        }
    }

    Ok(())
}

// Helper method to handle multi-trigger validation
async fn handle_multi_trigger(
    test: &TestDefinition,
    submit_client: &EvmSigningClient,
    trigger_id: TriggerId,
    submit_start_block: u64,
) -> Result<()> {
    let multi_trigger_service = test.multi_trigger_service.as_ref().unwrap();
    let multi_trigger_workflow = multi_trigger_service.workflows.values().next().unwrap();

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

    Ok(())
}
