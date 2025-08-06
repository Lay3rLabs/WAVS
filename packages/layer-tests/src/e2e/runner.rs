// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::{anyhow, Context};
use futures::{stream::FuturesUnordered, StreamExt};
use ordermap::OrderMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use wavs_types::{
    CosmosContractSubmission, EvmContractSubmission, Submit, Trigger, Workflow, WorkflowID,
};

use crate::e2e::config::Configs;
use crate::e2e::helpers::{
    change_service_for_test, wait_for_task_to_land_cosmos, wait_for_task_to_land_evm,
};
use crate::e2e::test_registry::CosmosContractCodeMap;
use crate::{
    e2e::{
        clients::Clients, components::ComponentSources, helpers::deploy_service_for_test,
        test_definition::TestDefinition, test_registry::TestRegistry,
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{SimpleEvmTriggerClient, TriggerId},
};

use super::test_definition::WorkflowDefinition;

/// Simplified test runner that leverages services directly attached to test definitions
pub struct Runner {
    configs: Arc<Configs>,
    clients: Arc<Clients>,
    registry: Arc<TestRegistry>,
    component_sources: Arc<ComponentSources>,
    cosmos_code_map: CosmosContractCodeMap,
}

impl Runner {
    pub fn new(
        configs: Configs,
        clients: Clients,
        registry: TestRegistry,
        component_sources: ComponentSources,
        cosmos_code_map: CosmosContractCodeMap,
    ) -> Self {
        Self {
            configs: Arc::new(configs),
            clients: Arc::new(clients),
            registry: Arc::new(registry),
            component_sources: Arc::new(component_sources),
            cosmos_code_map,
        }
    }

    /// Run all tests in the registry
    pub async fn run_tests(&self) {
        let test_groups = self.registry.list_all_grouped();

        for (group, group_tests) in test_groups {
            tracing::info!("Running group {} with {} tests", group, group_tests.len());
            let mut futures = FuturesUnordered::new();

            for test in group_tests {
                let clients = self.clients.clone();
                let component_sources = self.component_sources.clone();
                let mut test = test.clone();
                let cosmos_code_map = self.cosmos_code_map.clone();
                futures.push(async move {
                    self.execute_test(&mut test, clients, component_sources, cosmos_code_map)
                        .await
                });
            }

            while (futures.next().await).is_some() {}
        }
    }

    // Execute a single test with timings
    async fn execute_test(
        &self,
        test: &mut TestDefinition,
        clients: Arc<Clients>,
        component_sources: Arc<ComponentSources>,
        cosmos_code_map: CosmosContractCodeMap,
    ) {
        let test_name = test.name.clone();
        let start_time = Instant::now();

        run_test(
            &self.configs,
            test,
            &clients,
            &component_sources,
            cosmos_code_map,
        )
        .await
        .context(test.name.clone())
        .unwrap();
        let duration = start_time.elapsed();
        // This is a rough metric for debugging, since it can be interrupted by other async tasks
        tracing::info!(
            "Test {} passed (ran for {}ms)",
            test_name,
            duration.as_millis()
        );
    }
}

/// Run a single test
async fn run_test(
    configs: &Configs,
    test: &mut TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_code_map: CosmosContractCodeMap,
) -> anyhow::Result<()> {
    let aggregator_registered_service_ids = Arc::new(std::sync::Mutex::new(HashSet::new()));
    let service = deploy_service_for_test(
        configs,
        test,
        clients,
        component_sources,
        cosmos_code_map.clone(),
        aggregator_registered_service_ids,
    )
    .await;

    if let Some(change_service) = &mut test.change_service {
        change_service_for_test(
            change_service,
            &service,
            clients,
            component_sources,
            cosmos_code_map,
        )
        .await;
    }

    // Group workflows by trigger to handle multi-triggers
    let mut trigger_groups: OrderMap<&Trigger, Vec<(&WorkflowID, &Workflow)>> = OrderMap::new();

    for (workflow_id, workflow) in service.workflows.iter() {
        trigger_groups
            .entry(&workflow.trigger)
            .or_default()
            .push((workflow_id, workflow));
    }

    // Process each unique trigger once, then validate all associated workflows
    for (trigger, workflows_group) in trigger_groups {
        // Use the first workflow to execute the trigger
        let (first_workflow_id, _) = workflows_group[0];

        // Get the workflow data safely
        let first_workflow = test
            .workflows
            .get(first_workflow_id)
            .ok_or(anyhow!("Could not get workflow: {}", first_workflow_id))?;

        // Convert input data to bytes safely
        let input_bytes = first_workflow.input_data.to_bytes();

        // Execute the trigger once
        let trigger_id = match trigger {
            Trigger::EvmContractEvent {
                chain_name,
                address,
                event_hash: _,
            } => {
                let evm_client = clients.get_evm_client(chain_name);
                let client = SimpleEvmTriggerClient::new(evm_client, *address);

                client
                    .add_trigger(input_bytes.expect("EVM triggers require an input"))
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
                    .add_trigger(input_bytes.expect("Cosmos triggers require an input"))
                    .await?;

                TriggerId::new(trigger_id.u64())
            }
            Trigger::BlockInterval { .. } => TriggerId::new(1337),
            Trigger::Cron { .. } => TriggerId::new(1338),
            Trigger::Manual => unimplemented!("Manual trigger type is not implemented"),
        };

        // Validate all workflows associated with this trigger
        for (workflow_id, workflow) in workflows_group {
            let WorkflowDefinition {
                timeout,
                expected_output,
                ..
            } = &test.workflows.get(workflow_id).ok_or(anyhow!(
                "Could not get workflow definition from id: {}",
                workflow_id
            ))?;

            let signed_data = match &workflow.submit {
                Submit::Aggregator {
                    evm_contracts,
                    cosmos_contracts,
                    ..
                } => {
                    let mut signed_data = vec![];
                    let empty_vec = Vec::new();
                    let contracts = evm_contracts.as_ref().unwrap_or(&empty_vec);
                    for contract in contracts.iter() {
                        let EvmContractSubmission {
                            chain_name,
                            address,
                            ..
                        } = contract;

                        let client = clients.get_evm_client(chain_name);
                        let submit_start_block = client
                            .provider
                            .get_block_number()
                            .await
                            .map_err(|e| anyhow!("Failed to get block number: {}", e))?;

                        signed_data.push(
                            wait_for_task_to_land_evm(
                                client,
                                *address,
                                trigger_id,
                                submit_start_block,
                                *timeout,
                            )
                            .await?,
                        );
                    }

                    let empty_vec = Vec::new();
                    let contracts = cosmos_contracts.as_ref().unwrap_or(&empty_vec);
                    for contract in contracts.iter() {
                        let CosmosContractSubmission {
                            chain_name,
                            address,
                            ..
                        } = contract;

                        let client = clients.get_cosmos_client(chain_name).await;

                        signed_data.push(
                            wait_for_task_to_land_cosmos(
                                client,
                                address.clone(),
                                trigger_id,
                                *timeout,
                            )
                            .await?,
                        );
                    }

                    signed_data
                }
                Submit::None => unimplemented!("Submit::None is not implemented"),
            };

            for data in signed_data {
                expected_output.validate(test, clients, component_sources, &data.data)?;
            }
        }
    }

    Ok(())
}
