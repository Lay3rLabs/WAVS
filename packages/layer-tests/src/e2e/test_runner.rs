// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::{Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use std::sync::Arc;
use std::time::Instant;

use utils::context::AppContext;

use crate::e2e::{
    add_task::add_task, clients::Clients, test_definition::TestDefinition,
    test_registry::TestRegistry,
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
        let tests = self.registry.list_all();

        tracing::info!("Running {} tests", tests.len());

        self.ctx.rt.block_on(async {
            let mut futures = FuturesUnordered::new();

            // Flatten and submit all tests at once
            for test in tests {
                tracing::info!("Running test: {}", test.name);
                futures.push(self.execute_test(test, self.clients.clone()));
            }

            let mut failures = Vec::new();
            while let Some(result) = futures.next().await {
                if let Err(err) = result {
                    tracing::error!("Test failed: {:?}", err);
                    failures.push(err);
                }
            }

            if !failures.is_empty() {
                tracing::error!("{} test(s) failed", failures.len());
                std::process::exit(1);
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

    // Run tasks sequentially according to test definition
    for workflow in test.workflows.values() {
        add_task(
            clients,
            service_id.clone(),
            Some(workflow_id.to_string()),
            workflow.input_data.to_bytes(),
            submit_client.clone(),
            submit_start_block,
            true,
        )
        .await?;
    }

    Ok(())
}
