// src/e2e/test_runner.rs

use alloy_provider::Provider;
use anyhow::{anyhow, Context};
use futures::{stream::FuturesUnordered, StreamExt};
use ordermap::OrderMap;
use std::collections::HashMap;
use std::sync::Arc;
use wavs_types::{DeploymentResult, Submit, Trigger, Workflow, WorkflowID};

use crate::e2e::helpers::change_service_for_test;
use crate::e2e::report::TestReport;
use crate::e2e::service_managers::ServiceManagers;
use crate::e2e::test_definition::{
    AggregatorDefinition, ChangeServiceDefinition, SubmitDefinition,
};
use crate::e2e::test_registry::CosmosTriggerCodeMap;
use crate::{
    e2e::{
        clients::Clients, components::ComponentSources, test_definition::TestDefinition,
        test_registry::TestRegistry,
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{SimpleEvmTriggerClient, TriggerId},
};

use super::helpers::wait_for_task_to_land;
use super::test_definition::WorkflowDefinition;

/// Simplified test runner that leverages services directly attached to test definitions
pub struct Runner {
    clients: Arc<Clients>,
    registry: Arc<TestRegistry>,
    component_sources: Arc<ComponentSources>,
    service_managers: ServiceManagers,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
    report: TestReport,
}

impl Runner {
    pub fn new(
        clients: Clients,
        registry: TestRegistry,
        component_sources: ComponentSources,
        service_managers: ServiceManagers,
        cosmos_trigger_code_map: CosmosTriggerCodeMap,
        report: TestReport,
    ) -> Self {
        Self {
            clients: Arc::new(clients),
            registry: Arc::new(registry),
            component_sources: Arc::new(component_sources),
            service_managers,
            cosmos_trigger_code_map,
            report,
        }
    }

    /// Run all tests in the registry
    pub async fn run_tests(&self, mut all_services: HashMap<String, DeploymentResult>) {
        let test_groups = self.registry.list_all_grouped();

        for (group, mut group_tests) in test_groups {
            let services = group_tests
                .iter()
                .map(|test| all_services.get(&test.name).cloned().unwrap().service)
                .collect::<Vec<_>>();

            // This essentially deploys the services for the group
            // since it updates the services to "Active"
            // which is detected by wavs
            self.service_managers
                .update_services(&self.clients, services)
                .await;

            // However, we have some tests which demonstrate more specific service changes
            // and so we need to re-update those before we can proceed
            //
            // First we just deploy the service changes (contracts, components, etc.)
            let mut futures = FuturesUnordered::new();
            for test in group_tests.iter() {
                if let Some(change_service) = test.change_service.clone() {
                    let service = all_services.get(&test.name).cloned().unwrap().service;
                    futures.push(async move {
                        let mut service = service;
                        change_service_for_test(
                            &mut service,
                            change_service.clone(),
                            &self.clients,
                            &self.component_sources,
                            self.cosmos_trigger_code_map.clone(),
                        )
                        .await;
                        (service, change_service)
                    });
                }
            }

            // Then we need to deploy the update to service managers
            if futures.is_empty() {
                tracing::info!("No changes to services in group {}", group);
            } else {
                tracing::warn!("Running service changes for group {}", group);
                let mut services_to_change = Vec::new();
                while let Some((service, change_service)) = futures.next().await {
                    // update our local copy of the service while preserving submission handlers
                    let mut deployment_result = all_services.get(&service.name).cloned().unwrap();
                    deployment_result.service = service.clone();
                    all_services.insert(service.name.clone(), deployment_result);

                    // and the definition so that tests know what to look for
                    match change_service {
                        ChangeServiceDefinition::AddWorkflow {
                            workflow_id,
                            workflow,
                        } => {
                            // When a workflow is added, it includes a new submission contract
                            // Extract it from the service's workflow that was just added
                            let deployment_result = all_services.get_mut(&service.name).unwrap();
                            if let Some(service_workflow) =
                                deployment_result.service.workflows.get(&workflow_id)
                            {
                                if let Submit::Aggregator { component, .. } =
                                    &service_workflow.submit
                                {
                                    if let Some(contract_address_str) =
                                        component.config.get("contract_address")
                                    {
                                        if let Ok(address) = contract_address_str
                                            .parse::<alloy_primitives::Address>()
                                        {
                                            deployment_result
                                                .submission_handlers
                                                .insert(workflow_id.clone(), address);
                                        }
                                    }
                                }
                            }

                            group_tests
                                .iter_mut()
                                .find(|test| test.name == service.name)
                                .unwrap()
                                .workflows
                                .insert(workflow_id.clone(), workflow);
                        }
                        ChangeServiceDefinition::Component {
                            workflow_id,
                            component,
                        } => {
                            group_tests
                                .iter_mut()
                                .find(|test| test.name == service.name)
                                .unwrap()
                                .workflows
                                .get_mut(&workflow_id)
                                .unwrap()
                                .component = component;
                        }
                    }

                    services_to_change.push(service);
                }

                self.service_managers
                    .update_services(&self.clients, services_to_change)
                    .await;
            }

            // All services are now deployed and ready for the tests
            // From here on in we're strictly testing the trigger->execute->aggregate->submit flow
            tracing::info!("Running group {} with {} tests", group, group_tests.len());
            let mut futures = FuturesUnordered::new();

            for test in group_tests {
                let clients = self.clients.clone();
                let component_sources = self.component_sources.clone();
                let test = test.clone();
                let report = self.report.clone();
                let service = all_services.get(&test.name).cloned().unwrap();
                futures.push(async move {
                    self.execute_test(&test, service, clients, component_sources, report)
                        .await
                });
            }

            while (futures.next().await).is_some() {}
        }
    }

    // Execute a single test with timings
    async fn execute_test(
        &self,
        test: &TestDefinition,
        service: DeploymentResult,
        clients: Arc<Clients>,
        component_sources: Arc<ComponentSources>,
        report: TestReport,
    ) {
        report.start_test(test.name.clone());

        run_test(test, service, &clients, &component_sources)
            .await
            .context(test.name.clone())
            .unwrap();

        report.end_test(test.name.clone());
    }
}

/// Run a single test
async fn run_test(
    test: &TestDefinition,
    service: DeploymentResult,
    clients: &Clients,
    component_sources: &ComponentSources,
) -> anyhow::Result<()> {
    // Group workflows by trigger to handle multi-triggers
    let mut trigger_groups: OrderMap<&Trigger, Vec<(&WorkflowID, &Workflow)>> = OrderMap::new();

    for (workflow_id, workflow) in service.service.workflows.iter() {
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
                Submit::Aggregator { .. } => {
                    let workflow_def = test.workflows.get(workflow_id).ok_or_else(|| {
                        anyhow!("Could not get workflow definition from id: {}", workflow_id)
                    })?;

                    let SubmitDefinition::Aggregator { aggregators, .. } = &workflow_def.submit;
                    let AggregatorDefinition::ComponentBasedAggregator { chain_name, .. } =
                        &aggregators[0];

                    let client = clients.get_evm_client(chain_name);
                    let submit_start_block = client
                        .provider
                        .get_block_number()
                        .await
                        .map_err(|e| anyhow!("Failed to get block number: {}", e))?;

                    let submission_contract = service
                        .submission_handlers
                        .get(workflow_id)
                        .ok_or_else(|| {
                            anyhow!("No submission contract found for workflow {}", workflow_id)
                        })?;

                    vec![
                        wait_for_task_to_land(
                            client,
                            *submission_contract,
                            trigger_id,
                            submit_start_block,
                            *timeout,
                        )
                        .await?,
                    ]
                }
                Submit::None => unimplemented!("Submit::None is not implemented"),
            };

            for data in signed_data {
                expected_output.validate(test, clients, component_sources, &data.data)?;
            }
        }
    }

    clients
        .http_client
        .delete_service(vec![service.service.manager])
        .await?;

    Ok(())
}
