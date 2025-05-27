use alloy_provider::{ext::AnvilApi, Provider};
use alloy_sol_types::SolEvent;
use anyhow::{anyhow, Context, Result};
use std::{collections::BTreeMap, num::NonZero, sync::Arc, time::Duration};
use tokio::sync::Mutex;
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
        test_definition::{
            AggregatorDefinition, SubmitDefinition, TestDefinition, TriggerDefinition,
        },
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{
        example_submit::ISimpleSubmit::SignedData, example_trigger::SimpleTrigger,
        SimpleEvmSubmitClient, TriggerId,
    },
};

use super::{
    config::BLOCK_INTERVAL_DATA_PREFIX,
    test_definition::{
        CosmosTriggerDefinition, EvmTriggerDefinition, ExpectedOutput, WorkflowDefinition,
    },
    test_registry::CosmosTriggerCodeMap,
};

pub struct ServiceAndUri {
    pub service: Service,
    // The URI where the service is deployed
    // currently, in tests, this is the URL hosted on WAVS itself
    // but in production, it's typically IPFS
    pub uri: String,
}

/// Helper function to deploy a service for a test
pub async fn deploy_service_for_test(
    test: &mut TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
) -> ServiceAndUri {
    tracing::info!("Deploying service for test: {}", test.name);

    // Create unique service ID
    let service_id = ServiceID::new(Uuid::now_v7().as_hyphenated().to_string()).unwrap();
    let mut workflows = BTreeMap::new();

    // Deploy the service manager contract
    tracing::info!(
        "[{}] Deploying service manager on chain {}",
        test.name,
        test.service_manager_chain
    );
    let service_manager_address = deploy_service_manager(clients, &test.service_manager_chain)
        .await
        .unwrap();

    for (workflow_id, workflow) in test.workflows.iter_mut() {
        // Create components from test definition
        let component_source = component_sources
            .lookup
            .get(&workflow.component)
            .unwrap()
            .clone();

        let mut component = Component::new(component_source);
        component.permissions = Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        };

        tracing::info!("[{}] Creating submit from config", test.name);

        // Create the submit based on test configuration
        let submit = create_submit_from_config(&workflow.submit, clients, service_manager_address)
            .await
            .unwrap();

        let mut aggregators = vec![];
        for aggregator in &workflow.aggregators {
            let aggregator = match aggregator {
                AggregatorDefinition::NewEvmAggregatorSubmit { chain_name } => {
                    let submit = deploy_submit(clients, chain_name, service_manager_address)
                        .await
                        .unwrap();

                    if let Submit::EvmContract(evm_contract_submission) = submit {
                        Aggregator::Evm(evm_contract_submission)
                    } else {
                        panic!("EVM contract submission is expected from deploy a new evm aggregator submit")
                    }
                }
            };

            aggregators.push(aggregator);
        }

        tracing::info!("[{}] Creating trigger from config", test.name);
        // Create the trigger based on test configuration
        let trigger = create_trigger_from_config(
            workflow.trigger.clone(),
            clients,
            cosmos_trigger_code_map.clone(),
            Some(workflow),
        )
        .await;

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
                service_url: service_url.clone(),
            }),
        },
    )
    .await
    .unwrap();

    ServiceAndUri {
        service,
        uri: service_url,
    }
}

/// Create a trigger based on test configuration
pub async fn create_trigger_from_config(
    trigger_definition: TriggerDefinition,
    clients: &Clients,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
    workflow_definition: Option<&mut WorkflowDefinition>,
) -> Trigger {
    match trigger_definition {
        TriggerDefinition::NewEvmContract(evm_trigger_definition) => match evm_trigger_definition {
            EvmTriggerDefinition::SimpleContractEvent { chain_name } => {
                let client = clients.get_evm_client(&chain_name);

                // Deploy a new EVM trigger contract
                tracing::info!("Deploying EVM trigger contract on chain {}", chain_name);
                let contract = SimpleTrigger::deploy(client.provider.clone())
                    .await
                    .unwrap();
                let address = *contract.address();

                // Get the event hash
                let event_hash =
                    *crate::example_evm_client::example_trigger::NewTrigger::SIGNATURE_HASH;

                Trigger::EvmContractEvent {
                    chain_name: chain_name.clone(),
                    address,
                    event_hash: ByteArray::new(event_hash),
                }
            }
        },
        TriggerDefinition::NewCosmosContract(cosmos_trigger_definition) => {
            match cosmos_trigger_definition {
                CosmosTriggerDefinition::SimpleContractEvent { ref chain_name } => {
                    let client = clients.get_cosmos_client(chain_name).await;

                    // Get the code ID with better error handling
                    tracing::info!("Getting cosmos code ID for chain {}", chain_name);
                    let code_id = get_cosmos_code_id(
                        clients,
                        &cosmos_trigger_definition,
                        cosmos_trigger_code_map,
                    )
                    .await;

                    tracing::info!("Using cosmos code ID: {} for chain {}", code_id, chain_name);

                    // Deploy a new Cosmos trigger contract with better error handling
                    let contract_name = format!("simple_trigger_{}", Uuid::now_v7());
                    tracing::info!(
                        "Instantiating new contract '{}' with code ID {} on chain {}",
                        contract_name,
                        code_id,
                        chain_name
                    );

                    let contract =
                        SimpleCosmosTriggerClient::new_code_id(client, code_id, &contract_name)
                            .await
                            .unwrap();

                    tracing::info!(
                        "Successfully deployed cosmos contract at address: {}",
                        contract.contract_address
                    );

                    Trigger::CosmosContractEvent {
                        chain_name: chain_name.clone(),
                        address: contract.contract_address,
                        event_type: crate::example_cosmos_client::NewMessageEvent::KEY.to_string(),
                    }
                }
            }
        }
        TriggerDefinition::DeferredBlockIntervalTarget { chain_name } => {
            let workflow = workflow_definition
                .expect("Workflow not provided when using deferred block interval targets");

            let block_delay = 12;
            let current_block = if clients.evm_clients.contains_key(&chain_name) {
                let client = clients.get_evm_client(&chain_name);

                client.provider.get_block_number().await.unwrap()
            } else if clients.cosmos_client_pools.contains_key(&chain_name) {
                let client = clients.get_cosmos_client(&chain_name).await;

                client.querier.block_height().await.unwrap()
            } else {
                panic!("Chain is not configured: {}", chain_name)
            };
            let target_block = NonZero::new(current_block + block_delay).unwrap();

            workflow.expected_output = ExpectedOutput::Text(format!(
                "{}{}",
                BLOCK_INTERVAL_DATA_PREFIX,
                target_block.get()
            ));

            Trigger::BlockInterval {
                chain_name,
                n_blocks: NonZero::new(1u32).unwrap(),
                start_block: Some(target_block),
                end_block: Some(target_block),
            }
        }
        TriggerDefinition::Existing(trigger) => trigger.clone(),
    }
}

/// Create a submit based on test configuration
pub async fn create_submit_from_config(
    submit_config: &SubmitDefinition,
    clients: &Clients,
    service_manager_address: alloy_primitives::Address,
) -> Result<Submit> {
    match submit_config {
        SubmitDefinition::NewEvmContract { chain_name } => {
            deploy_submit(clients, chain_name, service_manager_address).await
        }
        SubmitDefinition::Existing(submit) => Ok(submit.clone()),
    }
}

/// Deploy service manager contract
pub async fn deploy_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
) -> Result<alloy_primitives::Address> {
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

/// Deploy submit contract and create a Submit from it
pub async fn get_cosmos_code_id(
    clients: &Clients,
    cosmos_trigger_definition: &CosmosTriggerDefinition,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
) -> u64 {
    // Get or insert the entry
    let entry = cosmos_trigger_code_map
        .entry(cosmos_trigger_definition.clone())
        .or_insert_with(|| Arc::new(Mutex::new(None)))
        .clone();

    // Lock the entry
    let mut guard = entry.lock().await;

    // If already uploaded, return the result
    if let Some(code_id) = *guard {
        return code_id;
    }

    // Upload since not cached
    let (chain_name, cosmos_bytecode) = match cosmos_trigger_definition {
        CosmosTriggerDefinition::SimpleContractEvent { chain_name } => {
            let wasm_path = workspace_path()
                .join("examples")
                .join("build")
                .join("contracts")
                .join("simple_example.wasm");

            if !wasm_path.exists() {
                panic!(
                    "Cosmos contract WASM file not found at: {}",
                    wasm_path.display()
                );
            }

            (chain_name, tokio::fs::read(&wasm_path).await.unwrap())
        }
    };

    tracing::info!(
        "Uploading cosmos wasm byte code ({} bytes) to chain {}",
        cosmos_bytecode.len(),
        chain_name
    );

    let client = clients.get_cosmos_client(chain_name).await;

    let (code_id, _) = client
        .contract_upload_file(cosmos_bytecode, None)
        .await
        .unwrap();

    tracing::info!(
        "Successfully uploaded WASM bytecode to chain {}, code_id: {}",
        chain_name,
        code_id
    );

    // Cache result and return
    *guard = Some(code_id);
    code_id
}

pub async fn wait_for_task_to_land(
    evm_submit_client: EvmSigningClient,
    address: alloy_primitives::Address,
    trigger_id: TriggerId,
    submit_start_block: u64,
    timeout: Duration,
) -> Result<SignedData> {
    let submit_client = SimpleEvmSubmitClient::new(evm_submit_client, address);

    tokio::time::timeout(timeout, async move {
        loop {
            let current_block = submit_client
                .evm_client
                .provider
                .get_block_number()
                .await
                .map_err(|e| anyhow!("Failed to get block number: {e}"))?;

            if current_block == submit_start_block {
                submit_client.evm_client.provider.evm_mine(None).await?;
            }

            if submit_client.trigger_validated(trigger_id).await {
                return submit_client
                    .signed_data(trigger_id)
                    .await
                    .map_err(|e| anyhow!("Failed to get signed data: {e}"));
            }

            tracing::debug!("Waiting for task response on trigger {}", trigger_id);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Timeout when waiting for task to land"))?
}
