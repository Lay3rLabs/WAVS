// src/e2e/test_runner.rs

use crate::deployment::ServiceDeployment;
use crate::e2e::config::Configs;
use crate::example_evm_client::example_trigger::ISimpleTrigger::TriggerInfo;
use crate::example_evm_client::example_trigger::NewTrigger;
use alloy_primitives::U256;
use alloy_provider::ext::AnvilApi;
use alloy_provider::Provider;
use alloy_sol_types::SolType;
use anyhow::{anyhow, bail, Context};
use futures::{stream::FuturesUnordered, StreamExt};
use ordermap::OrderMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use utils::alloy_helpers::SolidityEventFinder;
use wavs_types::{
    AtProtoAction, ChainKeyNamespace, SimulatedTriggerRequest, Submit, Trigger, TriggerData,
    Workflow, WorkflowId,
};

use crate::e2e::helpers::wait_for_hypercore_streams_to_finalize;
use crate::e2e::helpers::{change_service_for_test, cosmos_wait_for_task_to_land};
use crate::e2e::report::TestReport;
use crate::e2e::service_managers::ServiceManagers;
use crate::e2e::test_definition::{
    AggregatorDefinition, ChangeServiceDefinition, SubmitDefinition,
};
use crate::e2e::test_registry::CosmosCodeMap;
use crate::{
    e2e::{
        clients::Clients, components::ComponentSources, test_definition::TestDefinition,
        test_registry::TestRegistry,
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{LogSpamClient, SimpleEvmTriggerClient, TriggerId},
};
use serde_json::json;

use super::helpers::{evm_wait_for_task_to_land, simulate_anvil_reorg};
use super::test_definition::WorkflowDefinition;

/// Simplified test runner that leverages services directly attached to test definitions
pub struct Runner {
    configs: Arc<Configs>,
    clients: Arc<Clients>,
    registry: Arc<TestRegistry>,
    component_sources: Arc<ComponentSources>,
    service_managers: ServiceManagers,
    cosmos_code_map: CosmosCodeMap,
    report: TestReport,
}

/// Extract service handler address from an aggregator submit configuration
fn extract_aggregator_service_handler(submit: &Submit) -> Option<layer_climb::prelude::Address> {
    match submit {
        Submit::Aggregator { component, .. } => {
            component
                .config
                .get("service_handler")
                .and_then(|addr_str| {
                    match layer_climb::prelude::CosmosAddr::new_str(addr_str, None) {
                        Ok(cosmos_addr) => Some(layer_climb::prelude::Address::Cosmos(cosmos_addr)),
                        Err(_) => layer_climb::prelude::EvmAddr::new_str(addr_str)
                            .ok()
                            .map(layer_climb::prelude::Address::from),
                    }
                })
        }
        _ => None,
    }
}

impl Runner {
    pub fn new(
        configs: Configs,
        clients: Clients,
        registry: TestRegistry,
        component_sources: ComponentSources,
        service_managers: ServiceManagers,
        cosmos_code_map: CosmosCodeMap,
        report: TestReport,
    ) -> Self {
        Self {
            configs: Arc::new(configs),
            clients: Arc::new(clients),
            registry: Arc::new(registry),
            component_sources: Arc::new(component_sources),
            service_managers,
            cosmos_code_map,
            report,
        }
    }

    /// Run all tests in the registry
    pub async fn run_tests(&self, mut all_services: HashMap<String, ServiceDeployment>) {
        let test_groups = self.registry.list_all_grouped(self.configs.grouping);

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
                            self.cosmos_code_map.clone(),
                        )
                        .await;
                        (service, change_service)
                    });
                }
            }

            // Then we need to deploy the update to service managers
            if futures.is_empty() {
                tracing::info!("No changes to services in group {:?}", group);
            } else {
                tracing::warn!("Running service changes for group {:?}", group);
                let mut services_to_change = Vec::new();
                while let Some((service, change_service)) = futures.next().await {
                    // update our local copy of the service and handle changes
                    let service_deployment = all_services
                        .get_mut(&service.name)
                        .expect("Service should exist in all_services");

                    service_deployment.service = service.clone();

                    // and the definition so that tests know what to look for
                    match change_service {
                        ChangeServiceDefinition::AddWorkflow {
                            workflow_id,
                            workflow,
                        } => {
                            // When a workflow is added, it includes a new submission contract
                            // Extract it from the service's workflow that was just added
                            let submission_address = service_deployment
                                .service
                                .workflows
                                .get(&workflow_id)
                                .and_then(|workflow| {
                                    extract_aggregator_service_handler(&workflow.submit)
                                });

                            if let Some(address) = submission_address {
                                service_deployment
                                    .submission_handlers
                                    .insert(workflow_id.clone(), address);
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
            tracing::info!("Running group {:?} with {} tests", group, group_tests.len());
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
        service_deployment: ServiceDeployment,
        clients: Arc<Clients>,
        component_sources: Arc<ComponentSources>,
        report: TestReport,
    ) {
        report.start_test(test.name.clone());

        run_test(
            test,
            service_deployment,
            &clients,
            &component_sources,
            &self.registry,
        )
        .await
        .context(test.name.clone())
        .unwrap();

        report.end_test(test.name.clone());
    }
}

/// Run a single test
async fn run_test(
    test: &TestDefinition,
    service_deployment: ServiceDeployment,
    clients: &Clients,
    component_sources: &ComponentSources,
    registry: &TestRegistry,
) -> anyhow::Result<()> {
    // For multi-operator tests, wait for P2P mesh to form before triggering
    if test.multi_operator && clients.http_clients.len() > 1 {
        let expected_peers = clients.http_clients.len() - 1;
        tracing::info!(
            "Multi-operator test: waiting for P2P mesh formation ({} expected peers)",
            expected_peers
        );

        // Wait for all operators to have connected to peers
        for (idx, http_client) in clients.http_clients.iter().enumerate() {
            let status = http_client
                .wait_for_p2p_ready(expected_peers, Some(Duration::from_secs(30)))
                .await
                .map_err(|e| {
                    anyhow!(
                        "Operator {} P2P readiness check failed: {}. \
                         Multi-operator tests require P2P mesh to be ready.",
                        idx,
                        e
                    )
                })?;
            tracing::info!(
                "Operator {} P2P ready: {} connected peers",
                idx,
                status.connected_peers
            );
        }
    }

    // Group workflows by trigger to handle multi-triggers
    let mut trigger_groups: OrderMap<&Trigger, Vec<(&WorkflowId, &Workflow)>> = OrderMap::new();

    for (workflow_id, workflow) in service_deployment.service.workflows.iter() {
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
        let mut reorg_snapshot: Option<U256> = None;
        let trigger_ids = match trigger {
            Trigger::EvmContractEvent {
                chain,
                address,
                event_hash: _,
            } => {
                let evm_client = clients.get_evm_client(chain);
                let client = SimpleEvmTriggerClient::new(evm_client.clone(), *address);

                if first_workflow.expects_reorg() {
                    reorg_snapshot = Some(evm_client.provider.anvil_snapshot().await?);
                }
                let input = input_bytes.clone().expect("EVM triggers require an input");

                let spam_client = if first_workflow.trigger_execution.log_spam_count > 0 {
                    let address = super::helpers::deploy_log_spam_contract(clients, chain).await?;
                    let client = LogSpamClient::new(evm_client.clone(), address);
                    Some(client)
                } else {
                    None
                };

                #[derive(Clone, Copy, Debug)]
                enum TxKind {
                    Trigger,
                    Spam,
                }

                let mut pending: Vec<(TxKind, alloy_primitives::TxHash)> = Vec::new();

                let pending_trigger = client
                    .contract
                    .addTrigger(input.clone().into())
                    .send()
                    .await?;
                pending.push((TxKind::Trigger, *pending_trigger.tx_hash()));

                if let Some(spam_client) = &spam_client {
                    let spam_count = first_workflow.trigger_execution.log_spam_count as u64;
                    tracing::info!(
                        "Emitting {} bulk spam logs using LogSpam contract",
                        spam_count
                    );

                    // Use bulk emission to spam N logs in a single transaction
                    let spam_hash = spam_client.emit_spam(0, spam_count).await?;

                    tracing::info!("Bulk spam transaction sent: {:?}", spam_hash);
                    pending.push((TxKind::Spam, spam_hash));
                }

                let start = Instant::now();
                let mut receipts = Vec::new();

                while !pending.is_empty() {
                    let mut remaining = Vec::new();

                    for (kind, tx_hash) in pending.drain(..) {
                        tracing::debug!("Checking receipt for transaction: {:?}", tx_hash);
                        match evm_client.provider.get_transaction_receipt(tx_hash).await? {
                            Some(receipt) => {
                                receipts.push((kind, receipt));
                            }
                            None => remaining.push((kind, tx_hash)),
                        }
                    }

                    if start.elapsed() > Duration::from_secs(60) {
                        tracing::error!(
                            "Timeout waiting for transactions to be mined. Pending: {}, Mined: {}",
                            remaining.len(),
                            receipts.len()
                        );
                        bail!("Timed out waiting for transactions to be mined");
                    }

                    pending = remaining;
                }

                let mut trigger_ids = Vec::new();
                for (kind, receipt) in receipts {
                    if matches!(kind, TxKind::Trigger) {
                        if let Some(event) =
                            SolidityEventFinder::<NewTrigger>::solidity_event(&receipt)
                        {
                            let trigger_info = TriggerInfo::abi_decode(&event.triggerData)?;
                            trigger_ids.push(TriggerId::new(trigger_info.triggerId));
                        }
                    }
                }

                if trigger_ids.is_empty() {
                    bail!("Failed to obtain trigger id from transaction receipts");
                }

                tracing::info!(
                    "Successfully extracted {} trigger IDs: {:?}",
                    trigger_ids.len(),
                    trigger_ids
                );
                trigger_ids
            }
            Trigger::CosmosContractEvent {
                chain,
                address,
                event_type: _,
            } => {
                let client = SimpleCosmosTriggerClient::new(
                    clients.get_cosmos_client(chain).await,
                    address.clone().into(),
                );

                let trigger_id = client
                    .add_trigger(input_bytes.expect("Cosmos triggers require an input"))
                    .await?;

                vec![TriggerId::new(trigger_id.u64())]
            }
            Trigger::BlockInterval { .. } => vec![TriggerId::new(1337)],
            Trigger::Cron { .. } => vec![TriggerId::new(1338)],
            Trigger::AtProtoEvent { .. } => {
                let sequence: u64 = 1339;
                let trigger_id = TriggerId::new(sequence);

                let record_payload = input_bytes.clone().unwrap_or_default();
                let record_text = String::from_utf8_lossy(&record_payload).to_string();

                // Send simulated trigger to all WAVS instances
                for http_client in clients.http_clients.iter() {
                    let atproto_data = TriggerData::AtProtoEvent {
                        sequence: sequence as i64,
                        timestamp: 0,
                        repo: "did:example:alice".to_string(),
                        collection: "app.bsky.feed.post".to_string(),
                        rkey: "rkey-1".to_string(),
                        action: AtProtoAction::Create,
                        cid: Some("bafytestcid".to_string()),
                        record: Some(json!({ "text": record_text.clone() })),
                        rev: Some("rev-test".to_string()),
                        op_index: Some(0),
                    };

                    let req = SimulatedTriggerRequest {
                        service_id: service_deployment.service.id(),
                        workflow_id: first_workflow_id.clone(),
                        trigger: trigger.clone(),
                        data: atproto_data,
                        count: 1,
                        wait_for_completion: true,
                    };

                    http_client.simulate_trigger(req).await?;
                }

                vec![trigger_id]
            }
            Trigger::HypercoreAppend { feed_key } => {
                // Try to get the hypercore test client for this test
                let payload = input_bytes.clone().unwrap_or_default();

                tracing::info!("Hypercore trigger detected with feed_key: {}", feed_key);

                if let Some(hypercore_client) = registry.get_hypercore_client(&test.name) {
                    let client_feed_key = hypercore_client.feed_key();
                    tracing::info!(
                        "Using real hypercore feed for test '{}', client feed_key: {}, service feed_key: {}",
                        test.name,
                        client_feed_key,
                        feed_key
                    );

                    for (idx, http_client) in clients.http_clients.iter().enumerate() {
                        tracing::info!(
                            "Waiting for hypercore stream readiness on instance {} for feed_key {}",
                            idx,
                            feed_key
                        );
                        wait_for_hypercore_streams_to_finalize(
                            http_client,
                            feed_key,
                            Some(Duration::from_secs(30)),
                        )
                        .await
                        .context("Failed to wait for hypercore stream to finalize")?;
                    }

                    if test.multi_operator && clients.http_clients.len() > 1 {
                        let expected = clients.http_clients.len();
                        tracing::info!(
                            "Waiting for {} hyperswarm peers before appending",
                            expected
                        );
                        let connected = hypercore_client
                            .wait_for_peers(expected, Duration::from_secs(30))
                            .await?;
                        tracing::info!("Hypercore peers connected: {}", connected);
                    }

                    // Verify feed keys match
                    if client_feed_key != *feed_key {
                        tracing::error!(
                            "FEED KEY MISMATCH! Client has: {}, Service has: {}",
                            client_feed_key,
                            feed_key
                        );
                        return Err(anyhow::anyhow!(
                            "Feed key mismatch between client and service"
                        ));
                    }

                    // Append data to the hypercore feed
                    tracing::info!("Appending {} bytes to hypercore feed...", payload.len());
                    let index = hypercore_client.append(payload).await?;

                    vec![TriggerId::new(index)]
                } else {
                    // Fallback to simulated trigger for backward compatibility
                    tracing::warn!(
                        "No hypercore client found for test '{}', using simulated trigger",
                        test.name
                    );

                    let trigger_id = TriggerId::new(0);
                    let hypercore_data = TriggerData::HypercoreAppend {
                        feed_key: feed_key.clone(),
                        index: trigger_id.u64(),
                        data: payload,
                    };

                    let req = SimulatedTriggerRequest {
                        service_id: service_deployment.service.id(),
                        workflow_id: first_workflow_id.clone(),
                        trigger: trigger.clone(),
                        data: hypercore_data,
                        count: 1,
                        wait_for_completion: true,
                    };

                    let http_client = clients
                        .http_clients
                        .first()
                        .ok_or_else(|| anyhow!("No HTTP clients available"))?;
                    http_client.simulate_trigger(req).await?;

                    vec![trigger_id]
                }
            }
            Trigger::Manual => unimplemented!("Manual trigger type is not implemented"),
        };

        tracing::info!(
            "Starting workflow validation for {} workflows",
            workflows_group.len()
        );
        // Validate all workflows associated with this trigger
        for (workflow_id, workflow) in workflows_group {
            tracing::info!("Validating workflow: {}", workflow_id);
            let WorkflowDefinition {
                timeout,
                expected_output,
                ..
            } = &test.workflows.get(workflow_id).ok_or(anyhow!(
                "Could not get workflow definition from id: {}",
                workflow_id
            ))?;

            for trigger_id in trigger_ids.iter().copied() {
                tracing::info!(
                    "Processing trigger_id: {} for workflow: {}",
                    trigger_id,
                    workflow_id
                );
                let data = match &workflow.submit {
                    Submit::Aggregator { .. } => {
                        let workflow_def = test.workflows.get(workflow_id).ok_or_else(|| {
                            anyhow!("Could not get workflow definition from id: {}", workflow_id)
                        })?;

                        let SubmitDefinition::Aggregator(aggregator) = &workflow_def.submit;
                        let AggregatorDefinition::ComponentBasedAggregator { chain, .. } =
                            aggregator;

                        match chain.namespace.as_str() {
                            ChainKeyNamespace::COSMOS => {
                                let client = clients.get_cosmos_client(chain).await;
                                let submission_contract = service_deployment
                                    .submission_handlers
                                    .get(workflow_id)
                                    .ok_or_else(|| {
                                        anyhow!(
                                            "No submission contract found for workflow {}",
                                            workflow_id
                                        )
                                    })?;

                                let data = cosmos_wait_for_task_to_land(
                                    client,
                                    submission_contract.clone().try_into().unwrap(),
                                    trigger_id,
                                    *timeout,
                                )
                                .await?;

                                tracing::info!("Task result: {:?}", data);

                                data
                            }
                            ChainKeyNamespace::EVM => {
                                let client = clients.get_evm_client(chain);
                                tracing::info!(
                                    "Getting submit start block for workflow: {}",
                                    workflow_id
                                );
                                let submit_start_block =
                                    client.provider.get_block_number().await.map_err(|e| {
                                        anyhow!("Failed to get block number: {}", e)
                                    })?;
                                tracing::info!("Submit start block: {}", submit_start_block);

                                let submission_contract = service_deployment
                                    .submission_handlers
                                    .get(workflow_id)
                                    .ok_or_else(|| {
                                        anyhow!(
                                            "No submission contract found for workflow {}",
                                            workflow_id
                                        )
                                    })?;
                                tracing::info!(
                                    "Submission contract for workflow {}: {}",
                                    workflow_id,
                                    submission_contract
                                );

                                if first_workflow.expects_reorg() {
                                    tracing::info!("Test '{}' will simulate re-org", test.name);

                                    // Simulate re-org before waiting for task
                                    simulate_anvil_reorg(
                                        &client,
                                        reorg_snapshot.expect(
                                            "Expected a reorg snapshot when simulating reorg",
                                        ),
                                    )
                                    .await?;

                                    // Wait for task - should return empty data on error due to re-org
                                    tracing::info!(
                                        "Waiting for task to land after re-org for trigger_id: {}",
                                        trigger_id
                                    );
                                    let result = evm_wait_for_task_to_land(
                                        client,
                                        submission_contract.clone().try_into().unwrap(),
                                        trigger_id,
                                        submit_start_block,
                                        *timeout,
                                    )
                                    .await;

                                    match result {
                                        Ok(signed_data) => signed_data.data.to_vec(),
                                        // If we get an error (transaction dropped due to re-org),
                                        // return mocked signed data with empty content to match ExpectedOutput::Dropped
                                        Err(_) => Vec::new(),
                                    }
                                } else {
                                    tracing::info!(
                                        "Waiting for task to land (no re-org) for trigger_id: {}",
                                        trigger_id
                                    );
                                    let result = evm_wait_for_task_to_land(
                                        client,
                                        submission_contract.clone().try_into().unwrap(),
                                        trigger_id,
                                        submit_start_block,
                                        *timeout,
                                    )
                                    .await?;
                                    tracing::info!("Task result (no re-org): {:?}", result.data);
                                    result.data.to_vec()
                                }
                            }
                            _ => unimplemented!("Unsupported chain namespace for aggregator"),
                        }
                    }
                    Submit::None => unimplemented!("Submit::None is not implemented"),
                };

                tracing::info!("Validating expected output for workflow: {}", workflow_id);
                expected_output.validate(test, clients, component_sources, &data)?;
                tracing::info!(
                    "Successfully validated output for workflow: {}",
                    workflow_id
                );
            }
        }
        tracing::info!("Test completed successfully!");
    }

    // Wait for the aggregator submit callback to complete on all WAVS instances
    // before cleaning up the service. This ensures the after-submit callback
    // has finished writing to the KV store.
    // Only do this if:
    // 1. Any workflow uses an aggregator submit
    // 2. No workflow expects dropped output (e.g., reorg tests where submission is intentionally skipped)
    let has_aggregator = service_deployment
        .service
        .workflows
        .values()
        .any(|w| matches!(w.submit, Submit::Aggregator { .. }));

    let expects_dropped = test.workflows.values().any(|w| w.expects_reorg());

    if has_aggregator && !expects_dropped {
        let service_id = service_deployment.service.id().to_string();
        tracing::info!(
            "Waiting for submit callback to complete for service: {}",
            service_id
        );
        for (idx, http_client) in clients.http_clients.iter().enumerate() {
            http_client
                .wait_for_submit_callback(&service_id, None)
                .await
                .map_err(|e| {
                    anyhow!("Instance {} failed waiting for submit callback: {}", idx, e)
                })?;
            tracing::info!(
                "Submit callback completed on instance {} for service: {}",
                idx,
                service_id
            );
        }
    }

    tracing::info!(
        "Cleaning up service: {0:?}",
        service_deployment.service.manager
    );
    // Delete service from all WAVS instances
    for http_client in clients.http_clients.iter() {
        http_client
            .delete_service(vec![service_deployment.service.manager.clone()])
            .await?;
    }

    Ok(())
}
