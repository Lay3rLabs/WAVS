// src/e2e/helper.rs

use alloy_sol_types::SolEvent;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use uuid::Uuid;

use wavs_cli::command::deploy_service::{DeployService, DeployServiceArgs};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, EvmContractSubmission, Permissions,
    Service, ServiceID, ServiceManager, ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

use crate::{
    e2e::{
        clients::Clients,
        components::ComponentSources,
        test_definition::{SubmitConfig, TestDefinition, TriggerConfig},
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::example_trigger::SimpleTrigger,
};

/// Helper function to deploy a service for a test
pub async fn deploy_service_for_test(
    test: &TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
) -> Result<(Service, Option<Service>)> {
    // Create unique service ID
    let service_id = ServiceID::new(Uuid::now_v7().as_simple().to_string())?;

    // Create components from test definition
    let mut components = Vec::new();
    for component_name in &test.components {
        let component_source = component_sources
            .lookup
            .get(component_name)
            .context(format!(
                "Component source not found for {:?}",
                component_name
            ))?
            .clone();

        let mut component = Component::new(component_source);
        component.permissions = Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        };

        components.push(component);
    }

    // Make sure we have at least one component
    if components.is_empty() {
        bail!("No components specified for test: {}", test.name);
    }

    // Create the trigger based on test configuration
    let trigger = create_trigger_from_config(&test.trigger, clients).await?;

    // Create the submit based on test configuration
    let submit = create_submit_from_config(&test.submit, clients).await?;

    // Get the service manager address for the submit chain
    let submit_chain = get_chain_from_submit(&submit)?;
    let service_manager_address = deploy_service_manager(clients, &submit_chain).await?;

    // Create service workflows
    let workflow_id = WorkflowID::new("default")?;
    let workflow = Workflow {
        trigger: trigger.clone(), // Clone for possible use in multi-trigger service
        component: components[0].clone(),
        submit: submit.clone(),
        aggregators: Vec::new(),
    };

    let mut workflows = BTreeMap::new();
    workflows.insert(workflow_id, workflow);

    // Create the service
    let service = Service {
        id: service_id,
        name: test.name.clone(),
        workflows,
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain_name: submit_chain.clone(),
            address: service_manager_address,
        },
    };

    // Deploy the service using the CLI
    let submit_client = clients.get_evm_client(&submit_chain);
    DeployService::run(
        &clients.cli_ctx,
        submit_client.provider.clone(),
        DeployServiceArgs {
            service: service.clone(),
            service_url: None,
        },
    )
    .await?;

    // If this test uses multiple triggers, create a second service
    let multi_trigger_service = if test.use_multi_trigger {
        let multi_service_id = ServiceID::new(Uuid::now_v7().as_simple().to_string())?;

        // Create a new submit for the multi-trigger service
        let multi_submit = deploy_submit(clients, &submit_chain, service_manager_address).await?;

        // Create workflow for multi-trigger service (using same trigger)
        let multi_workflow_id = WorkflowID::new("multi")?;
        let multi_workflow = Workflow {
            trigger: trigger.clone(),
            component: components[0].clone(),
            submit: multi_submit,
            aggregators: Vec::new(),
        };

        let mut multi_workflows = BTreeMap::new();
        multi_workflows.insert(multi_workflow_id, multi_workflow);

        let multi_service = Service {
            id: multi_service_id,
            name: format!("{}_multi", test.name),
            workflows: multi_workflows,
            status: ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain_name: submit_chain.clone(),
                address: service_manager_address,
            },
        };

        // Deploy the multi-trigger service
        DeployService::run(
            &clients.cli_ctx,
            submit_client.provider.clone(),
            DeployServiceArgs {
                service: multi_service.clone(),
                service_url: None,
            },
        )
        .await?;

        Some(multi_service)
    } else {
        None
    };

    Ok((service, multi_trigger_service))
}

/// Create a trigger based on test configuration
pub async fn create_trigger_from_config(
    trigger_config: &TriggerConfig,
    clients: &Clients,
) -> Result<Trigger> {
    match trigger_config {
        TriggerConfig::EvmContract { chain_name } => {
            let client = clients.get_evm_client(chain_name);

            // Deploy a new EVM trigger contract
            let contract = SimpleTrigger::deploy(client.provider.clone()).await?;
            let address = *contract.address();

            // Get the event hash
            let event_hash =
                *crate::example_evm_client::example_trigger::NewTrigger::SIGNATURE_HASH;

            Ok(Trigger::EvmContractEvent {
                chain_name: chain_name.clone(),
                address,
                event_hash: ByteArray::new(event_hash),
            })
        }
        TriggerConfig::CosmosContract { chain_name } => {
            let client = clients.get_cosmos_client(chain_name).await;

            // Get the code ID for cosmos - this would need to be predeployed or deployed here
            let code_id = get_cosmos_code_id(clients, chain_name).await?;

            // Deploy a new Cosmos trigger contract
            let contract = SimpleCosmosTriggerClient::new_code_id(
                client,
                code_id,
                &format!("simple_trigger_{}", Uuid::now_v7()),
            )
            .await?;

            Ok(Trigger::CosmosContractEvent {
                chain_name: chain_name.clone(),
                address: contract.contract_address,
                event_type: crate::example_cosmos_client::NewMessageEvent::KEY.to_string(),
            })
        }
        TriggerConfig::BlockInterval {
            chain_name,
            n_blocks,
        } => Ok(Trigger::BlockInterval {
            chain_name: chain_name.clone(),
            n_blocks: std::num::NonZeroU32::new(*n_blocks)
                .unwrap_or(std::num::NonZeroU32::new(1).unwrap()),
        }),
        TriggerConfig::Cron { schedule } => Ok(Trigger::Cron {
            schedule: schedule.clone(),
            start_time: None,
            end_time: None,
        }),
        TriggerConfig::UseExisting { trigger } => Ok(trigger.clone()),
    }
}

/// Create a submit based on test configuration
pub async fn create_submit_from_config(
    submit_config: &SubmitConfig,
    clients: &Clients,
) -> Result<Submit> {
    match submit_config {
        SubmitConfig::EvmContract { chain_name } => {
            let service_manager_address = deploy_service_manager(clients, chain_name).await?;

            deploy_submit(clients, chain_name, service_manager_address).await
        }
        SubmitConfig::Aggregator { .. } => {
            // For aggregator, we use a URL instead of a contract address
            Ok(Submit::Aggregator {
                url: "http://127.0.0.1:8001".to_string(),
            })
        }
        SubmitConfig::None => Ok(Submit::None),
        SubmitConfig::UseExisting { submit } => Ok(submit.clone()),
    }
}

/// Get the chain name from a submit
pub fn get_chain_from_submit(submit: &Submit) -> Result<ChainName> {
    match submit {
        Submit::EvmContract(submission) => Ok(submission.chain_name.clone()),
        Submit::Aggregator { .. } => {
            // For aggregator, you would need to determine the chain from somewhere else
            bail!("Cannot determine chain name from aggregator submit")
        }
        Submit::None => bail!("None submit does not have a chain name"),
    }
}

/// Deploy service manager contract (re-exported from services.rs)
pub async fn deploy_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
) -> Result<alloy_primitives::Address> {
    // Re-export from services.rs or implement here
    let evm_client = clients.get_evm_client(chain_name);

    Ok(
        *crate::example_evm_client::example_service_manager::SimpleServiceManager::deploy(
            evm_client.provider.clone(),
        )
        .await?
        .address(),
    )
}

/// Deploy submit contract and create a Submit from it
pub async fn deploy_submit(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: alloy_primitives::Address,
) -> Result<Submit> {
    // Changed return type from Address to Submit
    // Deploy the contract
    let evm_client = clients.get_evm_client(chain_name);

    let address = *crate::example_evm_client::example_submit::SimpleSubmit::deploy(
        evm_client.provider.clone(),
        service_manager_address,
    )
    .await?
    .address();

    Ok(Submit::EvmContract(EvmContractSubmission {
        chain_name: chain_name.clone(),
        address,
        max_gas: None,
    }))
}

/// Get the Cosmos code ID (this would need to be predeployed or deployed here)
pub async fn get_cosmos_code_id(_clients: &Clients, _chain_namee: &ChainName) -> Result<u64> {
    // This would need to be implemented based on your cosmos code deployment
    // For now, return a hardcoded value as an example
    // In practice, you would need to either:
    // 1. Look up a predeployed code ID from a config
    // 2. Deploy the code and return the ID

    Ok(1) // Placeholder value
}
