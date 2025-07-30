use alloy_primitives::{Address, U256};
use alloy_provider::{ext::AnvilApi, Provider};
use alloy_sol_types::SolEvent;
use anyhow::{anyhow, Context, Result};
use std::{
    collections::{BTreeMap, HashSet},
    num::NonZero,
    sync::Arc,
    time::Duration,
};
use utils::{config::WAVS_ENV_PREFIX, evm_client::EvmSigningClient, filesystem::workspace_path};
use uuid::Uuid;

use wavs_cli::command::deploy_service::{DeployService, DeployServiceArgs, SetServiceUrlArgs};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, Permissions, Service, ServiceID,
    ServiceManager, ServiceStatus, SigningKeyResponse, Submit, Trigger, Workflow,
};

use crate::{
    e2e::{
        clients::Clients,
        components::ComponentSources,
        config::BLOCK_INTERVAL,
        test_definition::{
            AggregatorDefinition, ChangeServiceDefinition, ComponentDefinition, DeploymentResult,
            SubmitDefinition, TestDefinition, TriggerDefinition, WorkflowDeploymentResult,
        },
        test_registry::TestRegistry,
    },
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{
        example_service_manager::SimpleServiceManager, example_submit::ISimpleSubmit::SignedData,
        example_trigger::SimpleTrigger, SimpleEvmSubmitClient, TriggerId,
    },
};

use super::{
    test_definition::{CosmosTriggerDefinition, EvmTriggerDefinition, WorkflowDefinition},
    test_registry::CosmosTriggerCodeMap,
};

static SERVICE_UPDATE_TIMEOUT: Duration = Duration::from_secs(60 * 15);

pub async fn deploy_service_for_test(
    test: &mut TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
    aggregator_registered_service_ids: Arc<std::sync::Mutex<HashSet<ServiceID>>>,
) -> DeploymentResult {
    tracing::info!("Deploying service for test: {}", test.name);

    let mut workflows = BTreeMap::new();
    let mut submission_contracts = BTreeMap::new();

    // Deploy the service manager contract
    tracing::info!(
        "[{}] Deploying service manager on chain {}",
        test.name,
        test.service_manager_chain
    );
    let service_manager_address = deploy_service_manager(clients, &test.service_manager_chain)
        .await
        .unwrap();

    for (workflow_id, workflow_definition) in test.workflows.iter_mut() {
        let deployment_result = deploy_workflow(
            &test.name,
            workflow_definition,
            service_manager_address,
            clients,
            component_sources,
            cosmos_trigger_code_map.clone(),
        )
        .await;

        workflows.insert(workflow_id.clone(), deployment_result.workflow);
        submission_contracts.insert(workflow_id.clone(), deployment_result.submission_contract);
    }

    // Create the service in Paused state
    let mut service = Service {
        name: test.name.clone(),
        workflows,
        status: ServiceStatus::Paused,
        manager: ServiceManager::Evm {
            chain_name: test.service_manager_chain.clone(),
            address: service_manager_address,
        },
    };

    let submit_client = clients.get_evm_client(&test.service_manager_chain);

    tracing::info!("[{}] Deploying service: {}", test.name, service.id());

    // Save the service on WAVS endpoint (just a local test thing, real-world would be IPFS or similar)
    let service_url = DeployService::save_service(&clients.cli_ctx, &service)
        .await
        .unwrap();

    // First, register the service to the aggregator if needed
    for workflow in test.workflows.values() {
        if aggregator_registered_service_ids
            .lock()
            .unwrap()
            .insert(service.id())
        {
            let SubmitDefinition::Aggregator { url, .. } = &workflow.submit;
            TestRegistry::register_to_aggregator(url, &service)
                .await
                .unwrap();
        }
    }

    // Deploy the service on WAVS
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

    // give signer address some weight in the service manager
    let SigningKeyResponse::Secp256k1 { evm_address, .. } = clients
        .http_client
        .get_service_key(service.manager.clone())
        .await
        .unwrap();

    let service_manager =
        SimpleServiceManager::new(service_manager_address, submit_client.provider.clone());
    service_manager
        .setOperatorWeight(evm_address.parse().unwrap(), U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // activate the service
    // requires:
    // 1. Changing the service JSON to active
    // 2. Getting a URL for that updated JSON
    // 3. Setting that URI on the service manager
    // 4. waiting for that updated service to be observable on WAVS

    service.status = ServiceStatus::Active;

    let service_url = DeployService::save_service(&clients.cli_ctx, &service)
        .await
        .unwrap();

    service_manager
        .setServiceURI(service_url)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // wait until WAVS sees the new service
    clients
        .http_client
        .wait_for_service_update(&service, Some(SERVICE_UPDATE_TIMEOUT))
        .await
        .unwrap();

    DeploymentResult {
        service,
        submission_contracts,
    }
}

fn deploy_component(
    component_sources: &ComponentSources,
    component_definition: &ComponentDefinition,
    config_vars: BTreeMap<String, String>,
    env_vars: BTreeMap<String, String>,
) -> Component {
    // Create components from test definition
    let component_source = component_sources
        .lookup
        .get(&component_definition.name)
        .unwrap()
        .clone();

    let mut component = Component::new(component_source);
    component.permissions = Permissions {
        allowed_http_hosts: AllowedHostPermission::All,
        file_system: true,
    };
    component.config = config_vars;
    component.env_keys = env_vars.keys().cloned().collect();

    for (k, v) in env_vars.iter() {
        // NOTE: we should avoid collisions here
        std::env::set_var(format!("{}_{}", WAVS_ENV_PREFIX, k), v);
    }

    component
}

async fn deploy_workflow(
    test_name: &str,
    workflow_definition: &mut WorkflowDefinition,
    service_manager_address: alloy_primitives::Address,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
) -> WorkflowDeploymentResult {
    let component = deploy_component(
        component_sources,
        &workflow_definition.component,
        Default::default(),
        Default::default(),
    );

    tracing::info!("[{}] Creating submit from config", test_name);

    // Create the submit based on test configuration
    let chain_name = {
        let SubmitDefinition::Aggregator { aggregators, .. } = &workflow_definition.submit;
        let Some(first_agg) = aggregators.first() else {
            panic!("Expected at least one aggregator, but found none");
        };
        match first_agg {
            AggregatorDefinition::ComponentBasedAggregator { chain_name, .. } => chain_name,
        }
    };
    let submission_contract = deploy_submit_contract(clients, chain_name, service_manager_address)
        .await
        .unwrap();

    let submit = create_submit_from_config(
        &workflow_definition.submit,
        &submission_contract,
        Some(component_sources),
    )
    .await
    .unwrap();

    tracing::info!("[{}] Creating trigger from config", test_name);
    // Create the trigger based on test configuration
    let trigger = create_trigger_from_config(
        workflow_definition.trigger.clone(),
        clients,
        cosmos_trigger_code_map.clone(),
        Some(workflow_definition),
    )
    .await;

    // Create service workflows
    WorkflowDeploymentResult {
        workflow: Workflow {
            trigger: trigger.clone(), // Clone for possible use in multi-trigger service
            component,
            submit: submit.clone(),
        },
        submission_contract,
    }
}

/// Create a trigger based on test configuration
pub async fn create_trigger_from_config(
    trigger_definition: TriggerDefinition,
    clients: &Clients,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
    _workflow_definition: Option<&mut WorkflowDefinition>,
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
        TriggerDefinition::BlockInterval {
            chain_name,
            start_stop,
        } => match start_stop {
            false => Trigger::BlockInterval {
                chain_name,
                n_blocks: BLOCK_INTERVAL,
                start_block: None,
                end_block: None,
            },
            true => {
                let current_block = if clients.evm_clients.contains_key(&chain_name) {
                    let client = clients.get_evm_client(&chain_name);
                    client.provider.get_block_number().await.unwrap()
                } else if clients.cosmos_client_pools.contains_key(&chain_name) {
                    let client = clients.get_cosmos_client(&chain_name).await;
                    client.querier.block_height().await.unwrap()
                } else {
                    panic!("Chain is not configured: {}", chain_name)
                };

                let current_block = NonZero::new(current_block).unwrap();

                Trigger::BlockInterval {
                    chain_name,
                    n_blocks: BLOCK_INTERVAL,
                    start_block: Some(current_block),
                    end_block: Some(current_block),
                }
            }
        },
        TriggerDefinition::Existing(trigger) => trigger.clone(),
    }
}

/// Create a submit based on test configuration
pub async fn create_submit_from_config(
    submit_config: &SubmitDefinition,
    submission_contract: &Address,
    component_sources: Option<&ComponentSources>,
) -> Result<Submit> {
    match submit_config {
        SubmitDefinition::Aggregator { url, aggregators } => {
            if aggregators.is_empty() {
                return Err(anyhow!(
                    "Aggregator submit requires at least one aggregator"
                ));
            }

            // Take the first aggregator (for now we support one)
            let agg = &aggregators[0];

            match agg {
                AggregatorDefinition::ComponentBasedAggregator {
                    component: component_def,
                    ..
                } => {
                    if let Some(sources) = component_sources {
                        let mut config_vars = BTreeMap::new();
                        let env_vars = BTreeMap::new();

                        for (hardcoded_key, hardcoded_value) in
                            &component_def.configs_to_add.hardcoded
                        {
                            config_vars.insert(hardcoded_key.clone(), hardcoded_value.clone());
                        }

                        if component_def.configs_to_add.contract_address {
                            config_vars.insert(
                                "contract_address".to_string(),
                                format!("{:#x}", submission_contract),
                            );
                        }

                        let component =
                            deploy_component(sources, component_def, config_vars, env_vars);

                        Ok(Submit::Aggregator {
                            url: url.clone(),
                            component,
                        })
                    } else {
                        return Err(anyhow!(
                            "ComponentBasedAggregator requires component_sources"
                        ));
                    }
                }
            }
        }
    }
}

/// Deploy service manager contract
pub async fn deploy_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
) -> Result<alloy_primitives::Address> {
    let evm_client = clients.get_evm_client(chain_name);

    tracing::info!("Deploying service manager on chain {}", chain_name);

    let service_manager =
        crate::example_evm_client::example_service_manager::SimpleServiceManager::deploy(
            evm_client.provider.clone(),
        )
        .await
        .context("Failed to deploy service manager contract")?;

    service_manager
        .setLastCheckpointTotalWeight(U256::ONE)
        .send()
        .await?
        .watch()
        .await?;

    service_manager
        .setLastCheckpointThresholdWeight(U256::ONE)
        .send()
        .await?
        .watch()
        .await?;

    let address = *service_manager.address();
    tracing::info!("Service manager deployed at address: {}", address);

    Ok(address)
}

/// Deploy submit contract and return its address
pub async fn deploy_submit_contract(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: alloy_primitives::Address,
) -> Result<alloy_primitives::Address> {
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

    Ok(address)
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
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
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

/// Helper function to deploy a service for a test
pub async fn change_service_for_test(
    change_service: &mut ChangeServiceDefinition,
    old_service: &Service,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_trigger_code_map: CosmosTriggerCodeMap,
) {
    let mut new_service = old_service.clone();

    match change_service {
        ChangeServiceDefinition::Name(new_name) => {
            new_service.name = new_name.clone();
        }
        ChangeServiceDefinition::Component {
            workflow_id,
            component: component_definition,
        } => {
            let component = deploy_component(
                component_sources,
                component_definition,
                Default::default(),
                Default::default(),
            );
            let workflow = new_service
                .workflows
                .get_mut(workflow_id)
                .expect("Workflow not found in service");

            workflow.component = component;
        }
        ChangeServiceDefinition::AddWorkflow {
            workflow_id,
            workflow,
        } => {
            let deployment_result = deploy_workflow(
                workflow_id,
                workflow,
                new_service.manager.evm_address_unchecked(),
                clients,
                component_sources,
                cosmos_trigger_code_map,
            )
            .await;

            new_service
                .workflows
                .insert(workflow_id.clone(), deployment_result.workflow);
        }
    }

    let service_manager = match &old_service.manager {
        ServiceManager::Evm {
            chain_name,
            address,
        } => SimpleServiceManager::new(
            *address,
            clients.get_evm_client(chain_name).provider.clone(),
        ),
    };

    let url = DeployService::save_service(&clients.cli_ctx, &new_service)
        .await
        .unwrap();

    service_manager
        .setServiceURI(url)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // wait until WAVS sees the new service
    clients
        .http_client
        .wait_for_service_update(&new_service, Some(SERVICE_UPDATE_TIMEOUT))
        .await
        .unwrap();
}
