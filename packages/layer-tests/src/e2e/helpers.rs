use alloy_primitives::U256;
use alloy_provider::{ext::AnvilApi, Provider};
use alloy_sol_types::SolEvent;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    num::NonZero,
    sync::Arc,
    time::Duration,
};
use utils::{evm_client::EvmSigningClient, filesystem::workspace_path};
use uuid::Uuid;

use wavs_cli::command::deploy_service::{DeployService, DeployServiceArgs};
use wavs_types::{
    AllowedHostPermission, AnyChainConfig, ByteArray, ChainName, Component,
    CosmosContractSubmission, EvmContractSubmission, Permissions, Service, ServiceID,
    ServiceManager, ServiceStatus, SigningKeyResponse, Submit, Trigger, Workflow,
};

use crate::{
    e2e::{
        clients::Clients,
        components::ComponentSources,
        config::Configs,
        test_definition::{
            AggregatorDefinition, ChangeServiceDefinition, ComponentDefinition,
            CosmosContractDefinition, SubmitDefinition, TestDefinition, TriggerDefinition,
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
    config::BLOCK_INTERVAL_DATA_PREFIX,
    test_definition::{
        CosmosTriggerDefinition, EvmTriggerDefinition, ExpectedOutput, WorkflowDefinition,
    },
    test_registry::CosmosContractCodeMap,
};

static SERVICE_UPDATE_TIMEOUT: Duration = Duration::from_secs(60 * 15);

/// Helper function to deploy a service for a test
pub async fn deploy_service_for_test(
    configs: &Configs,
    test: &mut TestDefinition,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_code_map: CosmosContractCodeMap,
    aggregator_registered_service_ids: Arc<std::sync::Mutex<HashSet<ServiceID>>>,
) -> Service {
    tracing::info!("Deploying service for test: {}", test.name);

    // Create unique service ID
    let mut workflows = BTreeMap::new();

    // Deploy the service manager contract
    tracing::info!(
        "[{}] Deploying service manager on chain {}",
        test.name,
        test.service_manager_chain
    );
    let service_manager_address = deploy_service_manager(
        configs,
        clients,
        &test.service_manager_chain,
        &cosmos_code_map,
    )
    .await
    .unwrap();

    for (workflow_id, workflow_definition) in test.workflows.iter_mut() {
        let workflow = deploy_workflow(
            &test.name,
            workflow_definition,
            service_manager_address.clone(),
            clients,
            component_sources,
            cosmos_code_map.clone(),
        )
        .await;

        workflows.insert(workflow_id.clone(), workflow);
    }

    // Create the service in Paused state
    let mut service = Service {
        name: test.name.clone(),
        workflows,
        status: ServiceStatus::Paused,
        manager: match &service_manager_address {
            layer_climb::prelude::Address::Evm(address) => ServiceManager::Evm {
                chain_name: test.service_manager_chain.clone(),
                address: address.clone().into(),
            },

            layer_climb::prelude::Address::Cosmos { .. } => ServiceManager::Cosmos {
                chain_name: test.service_manager_chain.clone(),
                address: service_manager_address.clone(),
            },
        },
    };

    tracing::info!("[{}] Deploying service: {}", test.name, service.id());

    // Save the service on WAVS endpoint (just a local test thing, real-world would be IPFS or similar)
    let service_url = DeployService::save_service(&clients.cli_ctx, &service)
        .await
        .unwrap();

    match &service_manager_address {
        layer_climb::prelude::Address::Evm(service_manager_address) => {
            let submit_client = clients
                .evm_clients
                .get(&test.service_manager_chain)
                .unwrap();
            let service_manager = SimpleServiceManager::new(
                service_manager_address.clone().into(),
                submit_client.provider.clone(),
            );
            service_manager
                .setServiceURI(service_url.clone())
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
        }
        layer_climb::prelude::Address::Cosmos { .. } => {
            let client = clients.get_cosmos_client(&test.service_manager_chain).await;

            client
                .contract_execute(
                    &service_manager_address,
                    &wavs_types::contracts::cosmwasm::service_manager::ServiceManagerExecuteMessages::WavsSetServiceUri { service_uri: service_url },
                    Vec::new(),
                    None
                )
                .await
                .unwrap();
        }
    }

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
            set_service_url_args: None,
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

    match &service_manager_address {
        layer_climb::prelude::Address::Evm(service_manager_address) => {
            let submit_client = clients
                .evm_clients
                .get(&test.service_manager_chain)
                .unwrap();
            let service_manager = SimpleServiceManager::new(
                service_manager_address.clone().into(),
                submit_client.provider.clone(),
            );
            service_manager
                .setOperatorWeight(evm_address.parse().unwrap(), U256::ONE)
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
        }
        layer_climb::prelude::Address::Cosmos { .. } => {
            // TODO - set operator weight for cosmos
        }
    }

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

    match service_manager_address {
        layer_climb::prelude::Address::Evm(service_manager_address) => {
            let submit_client = clients
                .evm_clients
                .get(&test.service_manager_chain)
                .unwrap();
            let service_manager = SimpleServiceManager::new(
                service_manager_address.into(),
                submit_client.provider.clone(),
            );
            service_manager
                .setServiceURI(service_url)
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
        }
        layer_climb::prelude::Address::Cosmos { .. } => {
            let client = clients.get_cosmos_client(&test.service_manager_chain).await;

            client
                .contract_execute(
                    &service_manager_address,
                    &wavs_types::contracts::cosmwasm::service_manager::ServiceManagerExecuteMessages::WavsSetServiceUri { service_uri: service_url },
                    Vec::new(),
                    None
                )
                .await
                .unwrap();
        }
    }

    // wait until WAVS sees the new service
    clients
        .http_client
        .wait_for_service_update(&service, Some(SERVICE_UPDATE_TIMEOUT))
        .await
        .unwrap();

    service
}

fn deploy_component(
    component_sources: &ComponentSources,
    component_definition: &ComponentDefinition,
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
    component.config = component_definition.config_vars.clone();
    component.env_keys = component_definition.env_vars.keys().cloned().collect();

    for (k, v) in component_definition.env_vars.iter() {
        // NOTE: we should avoid collisions here
        std::env::set_var(k, v);
    }

    component
}

async fn deploy_workflow(
    test_name: &str,
    workflow_definition: &mut WorkflowDefinition,
    service_manager_address: layer_climb::prelude::Address,
    clients: &Clients,
    component_sources: &ComponentSources,
    cosmos_code_map: CosmosContractCodeMap,
) -> Workflow {
    let component = deploy_component(component_sources, &workflow_definition.component);

    tracing::info!("[{}] Creating submit from config", test_name);

    // Create the submit based on test configuration
    let chain_name = {
        let SubmitDefinition::Aggregator { aggregators, .. } = &workflow_definition.submit;
        match aggregators.first() {
            Some(AggregatorDefinition::NewEvmAggregatorSubmit { chain_name }) => chain_name.clone(),
            Some(AggregatorDefinition::NewCosmosAggregatorSubmit { chain_name }) => {
                chain_name.clone()
            }
            _ => {
                panic!("Expected at least one aggregator for submission, but found none");
            }
        }
    };
    let submission_contract = deploy_submit_contract(
        clients,
        &chain_name,
        service_manager_address,
        &cosmos_code_map,
    )
    .await
    .unwrap();
    let submit = create_submit_from_config(&workflow_definition.submit, &submission_contract)
        .await
        .unwrap();

    tracing::info!("[{}] Creating trigger from config", test_name);
    // Create the trigger based on test configuration
    let trigger = create_trigger_from_config(
        workflow_definition.trigger.clone(),
        clients,
        cosmos_code_map.clone(),
        Some(workflow_definition),
    )
    .await;

    // Create service workflows
    Workflow {
        trigger: trigger.clone(), // Clone for possible use in multi-trigger service
        component,
        submit: submit.clone(),
    }
}

/// Create a trigger based on test configuration
pub async fn create_trigger_from_config(
    trigger_definition: TriggerDefinition,
    clients: &Clients,
    cosmos_code_map: CosmosContractCodeMap,
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
                    let code_id = cosmos_code_map
                        .get_code_id(clients, &cosmos_trigger_definition.clone().into())
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

            let (current_block, block_delay) = if clients.evm_clients.contains_key(&chain_name) {
                let client = clients.get_evm_client(&chain_name);
                let current_block = client.provider.get_block_number().await.unwrap();
                let block_delay = 5;
                (current_block, block_delay)
            } else if clients.cosmos_client_pools.contains_key(&chain_name) {
                let client = clients.get_cosmos_client(&chain_name).await;
                let current_block = client.querier.block_height().await.unwrap();
                let block_delay = 12;
                (current_block, block_delay)
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
    submission_contract: &layer_climb::prelude::Address,
) -> Result<Submit> {
    match submit_config {
        SubmitDefinition::Aggregator { url, aggregators } => {
            let mut evm_contracts = Vec::new();
            let mut cosmos_contracts = Vec::new();

            for agg in aggregators {
                match agg {
                    AggregatorDefinition::NewEvmAggregatorSubmit { chain_name } => {
                        evm_contracts.push(EvmContractSubmission {
                            chain_name: chain_name.clone(),
                            address: submission_contract.clone().try_into().unwrap(),
                            max_gas: None,
                        });
                    }
                    AggregatorDefinition::NewCosmosAggregatorSubmit { chain_name } => {
                        cosmos_contracts.push(CosmosContractSubmission {
                            chain_name: chain_name.clone(),
                            address: submission_contract.clone(),
                            max_gas: None,
                        });
                    }
                }
            }

            Ok(Submit::Aggregator {
                url: url.clone(),
                component: None,
                evm_contracts: if evm_contracts.is_empty() {
                    None
                } else {
                    Some(evm_contracts)
                },
                cosmos_contracts: if cosmos_contracts.is_empty() {
                    None
                } else {
                    Some(cosmos_contracts)
                },
            })
        }
    }
}

/// Deploy service manager contract
pub async fn deploy_service_manager(
    configs: &Configs,
    clients: &Clients,
    chain_name: &ChainName,
    cosmos_code_map: &CosmosContractCodeMap,
) -> Result<layer_climb::prelude::Address> {
    match configs.chains.get_chain(chain_name).unwrap().unwrap() {
        AnyChainConfig::Evm(_) => deploy_evm_service_manager(clients, chain_name).await,
        AnyChainConfig::Cosmos(_) => {
            deploy_cosmos_service_manager(clients, chain_name, cosmos_code_map).await
        }
    }
}

async fn deploy_evm_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
) -> Result<layer_climb::prelude::Address> {
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

    Ok(address.into())
}

async fn deploy_cosmos_service_manager(
    clients: &Clients,
    chain_name: &ChainName,
    cosmos_code_map: &CosmosContractCodeMap,
) -> Result<layer_climb::prelude::Address> {
    let code_id = cosmos_code_map
        .get_code_id(
            clients,
            &CosmosContractDefinition::ServiceManager {
                chain_name: chain_name.clone(),
            },
        )
        .await;

    let client = clients.get_cosmos_client(chain_name).await;

    let (addr, _) = client
        .contract_instantiate(
            None,
            code_id,
            "service manager",
            &cosmwasm_std::Empty {},
            Vec::new(),
            None,
        )
        .await?;

    Ok(addr)
}

/// Deploy submit contract and return its address
pub async fn deploy_submit_contract(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: layer_climb::prelude::Address,
    cosmos_code_map: &CosmosContractCodeMap,
) -> Result<layer_climb::prelude::Address> {
    match service_manager_address {
        layer_climb::prelude::Address::Cosmos { .. } => {
            deploy_cosmos_submit_contract(
                clients,
                chain_name,
                service_manager_address,
                cosmos_code_map,
            )
            .await
        }
        layer_climb::prelude::Address::Evm(addr) => {
            deploy_evm_submit_contract(clients, chain_name, addr.into()).await
        }
    }
}

pub async fn deploy_evm_submit_contract(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: alloy_primitives::Address,
) -> Result<layer_climb::prelude::Address> {
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

    Ok(address.into())
}

pub async fn deploy_cosmos_submit_contract(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: layer_climb::prelude::Address,
    cosmos_code_map: &CosmosContractCodeMap,
) -> Result<layer_climb::prelude::Address> {
    let code_id = cosmos_code_map
        .get_code_id(
            clients,
            &CosmosContractDefinition::ServiceHandler {
                chain_name: chain_name.clone(),
            },
        )
        .await;

    let client = clients.get_cosmos_client(chain_name).await;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct InstantiateMsg {
        pub service_manager: String,
    }

    let (addr, _) = client
        .contract_instantiate(
            None,
            code_id,
            "service handler",
            &InstantiateMsg {
                service_manager: service_manager_address.to_string(),
            },
            Vec::new(),
            None,
        )
        .await?;

    Ok(addr)
}

impl CosmosContractCodeMap {
    /// Deploy submit contract and create a Submit from it
    pub async fn get_code_id(
        &self,
        clients: &Clients,
        cosmos_contract_definition: &CosmosContractDefinition,
    ) -> u64 {
        // Get or insert the entry
        let entry = self
            .0
            .entry(cosmos_contract_definition.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(None)))
            .clone();

        // Lock the entry
        let mut guard = entry.lock().await;

        // If already uploaded, return the result
        if let Some(code_id) = *guard {
            return code_id;
        }

        // Upload since not cached
        let (chain_name, wasm_path) = {
            match cosmos_contract_definition {
                CosmosContractDefinition::Trigger(
                    CosmosTriggerDefinition::SimpleContractEvent { chain_name },
                ) => (
                    chain_name,
                    workspace_path()
                        .join("examples")
                        .join("build")
                        .join("contracts")
                        .join("simple_example.wasm"),
                ),
                CosmosContractDefinition::ServiceManager { chain_name } => (
                    chain_name,
                    workspace_path()
                        .join("examples")
                        .join("build")
                        .join("contracts")
                        .join("mock_service_manager.wasm"),
                ),
                CosmosContractDefinition::ServiceHandler { chain_name } => (
                    chain_name,
                    workspace_path()
                        .join("examples")
                        .join("build")
                        .join("contracts")
                        .join("mock_service_handler.wasm"),
                ),
            }
        };

        if !wasm_path.exists() {
            panic!(
                "Cosmos contract WASM file not found at: {}",
                wasm_path.display()
            );
        }

        let cosmos_bytecode = tokio::fs::read(&wasm_path).await.unwrap();

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
    cosmos_code_map: CosmosContractCodeMap,
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
            let component = deploy_component(component_sources, component_definition);
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
            let deployed_workflow = deploy_workflow(
                workflow_id,
                workflow,
                new_service.manager.address(),
                clients,
                component_sources,
                cosmos_code_map,
            )
            .await;

            new_service
                .workflows
                .insert(workflow_id.clone(), deployed_workflow);
        }
    }

    let url = DeployService::save_service(&clients.cli_ctx, &new_service)
        .await
        .unwrap();

    match &old_service.manager {
        ServiceManager::Evm {
            chain_name,
            address,
        } => {
            let service_manager = SimpleServiceManager::new(
                *address,
                clients.get_evm_client(chain_name).provider.clone(),
            );
            service_manager
                .setServiceURI(url)
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
        }

        ServiceManager::Cosmos {
            chain_name,
            address,
        } => {
            // TODO - change URI for cosmos
            tracing::warn!(
                "Changing service URI for Cosmos is not implemented yet: {chain_name}, {address}"
            );
        }
    };

    // wait until WAVS sees the new service
    clients
        .http_client
        .wait_for_service_update(&new_service, Some(SERVICE_UPDATE_TIMEOUT))
        .await
        .unwrap();
}
