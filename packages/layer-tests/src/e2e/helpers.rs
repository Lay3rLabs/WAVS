use alloy_provider::{ext::AnvilApi, Provider};
use alloy_sol_types::SolEvent;
use anyhow::{bail, Context, Result};
use std::{collections::BTreeMap, time::Duration};
use utils::{evm_client::EvmSigningClient, filesystem::workspace_path};
use uuid::Uuid;

use wavs_cli::command::deploy_service::{DeployService, DeployServiceArgs, SetServiceUrlArgs};
use wavs_types::{
    Aggregator, AllowedHostPermission, ByteArray, ChainName, Component, EvmContractSubmission,
    Permissions, Service, ServiceID, ServiceManager, ServiceStatus, Submit, Trigger, Workflow,
};

use crate::{
    e2e::{
        clients::Clients,
        components::ComponentSources,
        test_definition::{AggregatorConfig, SubmitConfig, TestDefinition, TriggerConfig},
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{
        example_submit::ISimpleSubmit::SignedData, example_trigger::SimpleTrigger,
        SimpleEvmSubmitClient, TriggerId,
    },
};

/// Helper function to deploy a service for a test
pub async fn deploy_service_for_test(
    test: &TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
) -> Result<Service> {
    tracing::info!("Deploying service for test: {}", test.name);

    // Create unique service ID
    let service_id = ServiceID::new(Uuid::now_v7().as_hyphenated().to_string())?;
    let mut workflows = BTreeMap::new();

    // Deploy the service manager contract
    tracing::info!(
        "[{}] Deploying service manager on chain {}",
        test.name,
        test.service_manager_chain
    );
    let service_manager_address = deploy_service_manager(clients, &test.service_manager_chain)
        .await
        .context("Failed to deploy service manager")?;

    for (workflow_id, workflow) in &test.workflows {
        // Create components from test definition
        let component_source = component_sources
            .lookup
            .get(&workflow.component)
            .context(format!(
                "Component source not found for {:?}",
                workflow.component
            ))?
            .clone();

        let mut component = Component::new(component_source);
        component.permissions = Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        };

        tracing::info!("[{}] Creating trigger from config", test.name);
        // Create the trigger based on test configuration
        let trigger = create_trigger_from_config(&workflow.trigger, clients)
            .await
            .context("Failed to create trigger")?;

        tracing::info!("[{}] Creating submit from config", test.name);

        // Create the submit based on test configuration
        let submit = create_submit_from_config(&workflow.submit, clients, service_manager_address)
            .await
            .context("Failed to create submit")?;

        let mut aggregators = vec![];
        for aggregator in &workflow.aggregators {
            let aggregator = match aggregator {
                AggregatorConfig::NewEvmAggregatorSubmit { chain_name } => {
                    let submit =
                        deploy_submit(clients, chain_name, service_manager_address).await?;

                    if let Submit::EvmContract(evm_contract_submission) = submit {
                        Aggregator::Evm(evm_contract_submission)
                    } else {
                        bail!("EVM contract submission is expected from deploy a new evem aggregator submit")
                    }
                }
                AggregatorConfig::Aggregator(aggregator) => aggregator.clone(),
            };

            aggregators.push(aggregator);
        }

        // Create service workflows
        let workflow = Workflow {
            trigger: trigger.clone(), // Clone for possible use in multi-trigger service
            component,
            submit: submit.clone(),
            aggregators,
        };

        workflows.insert(workflow_id.clone(), workflow);
    }

    // Create the service
    let service = Service {
        id: service_id,
        name: test.name.clone(),
        workflows,
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain_name: test.service_manager_chain.clone(),
            address: service_manager_address,
        },
    };

    // Deploy the service using the CLI
    let submit_client = clients.get_evm_client(&test.service_manager_chain);

    tracing::info!("[{}] Deploying service: {}", test.name, service.id);

    // Deploy the service
    let service_url = DeployService::save_service(&clients.cli_ctx, &service)
        .await
        .unwrap();
    DeployService::run(
        &clients.cli_ctx,
        DeployServiceArgs {
            service: service.clone(),
            set_service_url_args: Some(SetServiceUrlArgs {
                provider: submit_client.provider.clone(),
                service_url,
            }),
        },
    )
    .await
    .context("Failed to deploy service")?;

    Ok(service)
}

/// Create a trigger based on test configuration
pub async fn create_trigger_from_config(
    trigger_config: &TriggerConfig,
    clients: &Clients,
) -> Result<Trigger> {
    match trigger_config {
        TriggerConfig::NewEvmContract { chain_name } => {
            let client = clients.get_evm_client(chain_name);

            // Deploy a new EVM trigger contract
            tracing::info!("Deploying EVM trigger contract on chain {}", chain_name);
            let contract = SimpleTrigger::deploy(client.provider.clone())
                .await
                .context("Failed to deploy EVM trigger contract")?;
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
        TriggerConfig::NewCosmosContract { chain_name } => {
            let client = clients.get_cosmos_client(chain_name).await;

            // Get the code ID with better error handling
            tracing::info!("Getting cosmos code ID for chain {}", chain_name);
            let code_id = get_cosmos_code_id(clients, chain_name)
                .await
                .context(format!(
                    "Failed to get cosmos code ID for chain {}",
                    chain_name
                ))?;

            tracing::info!("Using cosmos code ID: {} for chain {}", code_id, chain_name);

            // Deploy a new Cosmos trigger contract with better error handling
            let contract_name = format!("simple_trigger_{}", Uuid::now_v7());
            tracing::info!(
                "Instantiating new contract '{}' with code ID {} on chain {}",
                contract_name,
                code_id,
                chain_name
            );

            let contract = SimpleCosmosTriggerClient::new_code_id(client, code_id, &contract_name)
                .await
                .context(format!(
                    "Failed to instantiate cosmos contract with code ID {} on chain {}",
                    code_id, chain_name
                ))?;

            tracing::info!(
                "Successfully deployed cosmos contract at address: {}",
                contract.contract_address
            );

            Ok(Trigger::CosmosContractEvent {
                chain_name: chain_name.clone(),
                address: contract.contract_address,
                event_type: crate::example_cosmos_client::NewMessageEvent::KEY.to_string(),
            })
        }
        TriggerConfig::Trigger(trigger) => Ok(trigger.clone()),
    }
}

/// Create a submit based on test configuration
pub async fn create_submit_from_config(
    submit_config: &SubmitConfig,
    clients: &Clients,
    service_manager_address: alloy_primitives::Address,
) -> Result<Submit> {
    match submit_config {
        SubmitConfig::NewEvmContract { chain_name } => {
            deploy_submit(clients, chain_name, service_manager_address).await
        }
        SubmitConfig::Submit(submit) => Ok(submit.clone()),
    }
}

/// Deploy service manager contract (re-exported from services.rs)
pub async fn deploy_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
) -> Result<alloy_primitives::Address> {
    // Re-export from services.rs or implement here
    let evm_client = clients.get_evm_client(chain_name);

    tracing::info!("Deploying service manager on chain {}", chain_name);

    let result = crate::example_evm_client::example_service_manager::SimpleServiceManager::deploy(
        evm_client.provider.clone(),
    )
    .await
    .context("Failed to deploy service manager contract")?;

    let address = *result.address();
    tracing::info!("Service manager deployed at address: {}", address);

    Ok(address)
}

/// Deploy submit contract and create a Submit from it
pub async fn deploy_submit(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: alloy_primitives::Address,
) -> Result<Submit> {
    // Deploy the contract
    let evm_client = clients.get_evm_client(chain_name);

    tracing::info!(
        "Deploying submit contract on chain {} with service manager: {}",
        chain_name,
        service_manager_address
    );

    let result = crate::example_evm_client::example_submit::SimpleSubmit::deploy(
        evm_client.provider.clone(),
        service_manager_address,
    )
    .await
    .context("Failed to deploy submit contract")?;

    let address = *result.address();
    tracing::info!("Submit contract deployed at address: {}", address);

    Ok(Submit::EvmContract(EvmContractSubmission {
        chain_name: chain_name.clone(),
        address,
        max_gas: None,
    }))
}

pub async fn get_cosmos_code_id(clients: &Clients, chain_name: &ChainName) -> Result<u64> {
    // Check if the WASM file exists
    let wasm_path = workspace_path()
        .join("examples")
        .join("build")
        .join("contracts")
        .join("simple_example.wasm");

    if !wasm_path.exists() {
        return Err(anyhow::anyhow!(
            "Cosmos contract WASM file not found at: {}",
            wasm_path.display()
        ));
    }

    // Read the WASM bytecode
    let cosmos_bytecode = tokio::fs::read(&wasm_path).await?;

    tracing::info!(
        "Uploading cosmos wasm byte code ({} bytes) to chain {}",
        cosmos_bytecode.len(),
        chain_name
    );

    // Get a cosmos client for the chain
    let client = clients.get_cosmos_client(chain_name).await;

    // Upload the contract and get the real code ID
    let (code_id, _) = client
        .contract_upload_file(cosmos_bytecode, None)
        .await
        .context(format!(
            "Failed to upload WASM code to chain {}",
            chain_name
        ))?;

    tracing::info!(
        "Successfully uploaded WASM bytecode to chain {}, code_id: {}",
        chain_name,
        code_id
    );

    Ok(code_id)
}

pub async fn wait_for_task_to_land(
    evm_submit_client: EvmSigningClient,
    address: alloy_primitives::Address,
    trigger_id: TriggerId,
    submit_start_block: u64,
) -> SignedData {
    let submit_client = SimpleEvmSubmitClient::new(evm_submit_client, address);

    tokio::time::timeout(Duration::from_secs(5), async move {
        loop {
            if submit_client
                .evm_client
                .provider
                .get_block_number()
                .await
                .unwrap()
                == submit_start_block
            {
                submit_client
                    .evm_client
                    .provider
                    .evm_mine(None)
                    .await
                    .unwrap();
            }
            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    return submit_client.signed_data(trigger_id).await.unwrap();
                }
                false => {
                    tracing::debug!("Waiting for task response on trigger {}", trigger_id);
                }
            }

            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap()
}
