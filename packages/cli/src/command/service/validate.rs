use alloy_provider::{Provider, RootProvider};
use anyhow::Result;
use layer_climb::{prelude::Address,
    querier::QueryClient as CosmosQueryClient,
};
use reqwest::Client;
use std::collections::HashMap;
use wavs_types::{
    Aggregator, ChainName, EvmContractSubmission, ServiceManager, Trigger, WorkflowID,
};

/// Validate a workflow trigger using a Cosmos query client
pub async fn validate_workflow_trigger(
    workflow_id: &WorkflowID,
    trigger: &Trigger,
    query_client: &CosmosQueryClient,
    errors: &mut Vec<String>,
) {
    match trigger {
        Trigger::CosmosContractEvent {
            address,
            chain_name,
            event_type,
        } => {
            // Use same validation as in set_cosmos_trigger
            if let Err(err) = query_client
                .chain_config
                .parse_address(address.to_string().as_ref())
            {
                errors.push(format!(
                    "Workflow '{}' has an invalid Cosmos address format for chain {}: {}",
                    workflow_id, chain_name, err
                ));
            }

            // Validate event type
            if event_type.is_empty() {
                errors.push(format!(
                    "Workflow '{}' has an empty event type in Cosmos trigger",
                    workflow_id
                ));
            }
        }
        _ => {
            // For other trigger types, this has already been validated in ServiceJson::validate
        }
    }
}

/// Check registry availability for component services
pub async fn validate_registry_availability(registry_url: &str, errors: &mut Vec<String>) {
    // Create HTTP client with reasonable timeouts
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    // Construct the URL for the app endpoint
    let app_url = format!("{}/app", registry_url);

    // Try to fetch the app endpoint using HTTP GET request to check availability
    let result = match client.get(&app_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(())
            } else if response.status().is_client_error() {
                // 4xx status usually means endpoint doesn't exist or access denied
                Err(format!(
                    "Registry app endpoint returned client error (status: {})",
                    response.status()
                ))
            } else {
                // 5xx or other unexpected status
                Err(format!(
                    "Registry returned error status: {}",
                    response.status()
                ))
            }
        }
        Err(err) => {
            if err.is_timeout() {
                Err("Connection to registry timed out".to_string())
            } else if err.is_connect() {
                Err("Failed to connect to registry".to_string())
            } else {
                Err(format!("Network error: {}", err))
            }
        }
    };

    // Add error message if availability check failed
    if let Err(msg) = result {
        errors.push(format!("Registry availability check failed: {}", msg));
    }
}

/// Validation helper to check if contracts referenced in triggers exist on-chain
pub async fn validate_contracts_exist(
    service_name: &str,
    triggers: Vec<(&WorkflowID, &Trigger)>,
    aggregators: Vec<(&WorkflowID, &Aggregator)>,
    service_manager: Option<&ServiceManager>,
    evm_providers: &HashMap<ChainName, RootProvider>,
    cosmos_clients: &HashMap<ChainName, CosmosQueryClient>,
    errors: &mut Vec<String>,
) -> Result<()> {
    // Track which contracts we've already checked to avoid duplicate checks
    let mut checked_evm_contracts = HashMap::new();
    let mut checked_cosmos_contracts = HashMap::new();

    // Check all trigger contracts
    for (workflow_id, trigger) in triggers {
        match trigger {
            Trigger::EvmContractEvent {
                address,
                chain_name,
                ..
            } => {
                // Check if we have a provider for this chain
                if let Some(provider) = evm_providers.get(chain_name) {
                    // Only check each contract once per chain
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_evm_contracts.entry(key)
                    {
                        let context =
                            format!("Service {} workflow {} trigger", service_name, workflow_id);
                        match check_evm_contract_exists(address, provider, errors, &context).await {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking EVM contract for workflow {}: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check EVM contract for workflow {} - no provider configured for chain {}",
                        workflow_id, chain_name
                    ));
                }
            }
            Trigger::CosmosContractEvent {
                address,
                chain_name,
                ..
            } => {
                // Check if we have a query client for this chain
                if let Some(client) = cosmos_clients.get(chain_name) {
                    // Only check each contract once per chain
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_cosmos_contracts.entry(key)
                    {
                        let context =
                            format!("Service {} workflow {} trigger", service_name, workflow_id);
                        match check_cosmos_contract_exists(address, client, errors, &context).await
                        {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking Cosmos contract for workflow {}: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check Cosmos contract for workflow {} - no client configured for chain {}",
                        workflow_id, chain_name
                    ));
                }
            }
            // Other trigger types don't need contract validation
            Trigger::Cron { .. } | Trigger::Manual | Trigger::BlockInterval { .. } => {}
        }
    }

    // Check all aggregators
    for (workflow_id, aggregator) in aggregators {
        match aggregator {
            Aggregator::Evm(EvmContractSubmission {
                chain_name,
                address,
                ..
            }) => {
                // Check if we have a provider for this chain
                if let Some(provider) = evm_providers.get(chain_name) {
                    // Only check each contract once per chain
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_evm_contracts.entry(key)
                    {
                        let context =
                            format!("Service {} workflow {} submit", service_name, workflow_id);
                        match check_evm_contract_exists(address, provider, errors, &context).await {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking EVM contract for workflow {} submit: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check EVM contract for workflow {} submit - no provider configured for chain {}",
                        workflow_id, chain_name
                    ));
                }
            }
        }
    }

    if let Some(service_manager) = service_manager {
        match service_manager {
            ServiceManager::Evm {
                chain_name,
                address,
            } => {
                if let Some(provider) = evm_providers.get(chain_name) {
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_evm_contracts.entry(key)
                    {
                        let context = format!("Service {} manager", service_name);
                        match check_evm_contract_exists(address, provider, errors, &context).await {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking EVM contract for service manager: {}",
                                    err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check service manager contract - no provider configured for chain {}",
                        chain_name
                    ));
                }
            }
        };
    }

    Ok(())
}

/// Check if an EVM contract exists at the specified address
pub async fn check_evm_contract_exists(
    address: &alloy_primitives::Address,
    provider: &RootProvider,
    errors: &mut Vec<String>,
    context: &str,
) -> Result<bool> {
    // Get the code at the address - if empty, no contract exists
    match provider.get_code_at(*address).await {
        Ok(code) => {
            let exists = !code.is_empty();
            if !exists {
                errors.push(format!(
                    "{}: EVM address {} has no contract deployed on chain (empty bytecode)",
                    context, address
                ));
            }
            Ok(exists)
        }
        Err(err) => {
            errors.push(format!(
                "{}: Failed to check EVM contract at {}: {} (RPC connection issue)",
                context, address, err
            ));
            Err(err.into())
        }
    }
}

/// Check if a Cosmos contract exists at the specified address
pub async fn check_cosmos_contract_exists(
    address: &Address,
    query_client: &CosmosQueryClient,
    errors: &mut Vec<String>,
    context: &str,
) -> Result<bool> {
    // Query contract info to check if it exists
    // This uses CosmWasm-specific query if supported by the chain
    let result = query_client.contract_info(address).await;

    match result {
        Ok(_) => {
            // Contract exists and returned info
            Ok(true)
        }
        Err(err) => {
            errors.push(format!(
                "{}: Failed to check Cosmos contract at {}: {}",
                context, address, err
            ));
            Err(err)
        }
    }
}
