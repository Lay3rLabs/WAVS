mod types;
mod validate;

#[cfg(test)]
mod tests;

pub use types::{
    ChainType, ComponentContext, ComponentOperationResult, EvmManagerResult, ServiceInitResult,
    ServiceValidationResult, UpdateStatusResult, WorkflowAddResult, WorkflowDeleteResult,
    WorkflowSetSubmitNoneResult, WorkflowTriggerResult,
};
pub use validate::{
    check_cosmos_contract_exists, check_evm_contract_exists, validate_contracts_exist,
    validate_registry_availability, validate_workflow_trigger,
};

use alloy_json_abi::Event;
use alloy_provider::Provider;
use anyhow::{anyhow, Context as _, Result};
use layer_climb::querier::QueryClient as CosmosQueryClient;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::Write,
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
};
use utils::{config::WAVS_ENV_PREFIX, service::fetch_bytes, wkg::WkgClient};
use uuid::Uuid;
use wavs_types::{
    AggregatorBuilder, AllowedHostPermission, AnyChainConfig, AtProtoAction, ByteArray, ChainKey,
    Component, ComponentBuilder, ComponentDigest, ComponentSource, Registry, ServiceBuilder,
    ServiceManager, ServiceManagerBuilder, ServiceStatus, SignatureKind, Submit, SubmitBuilder,
    Timestamp, Trigger, TriggerBuilder, WorkflowBuilder, WorkflowId,
};

use crate::{
    args::{
        ComponentCommand, ManagerCommand, ServiceCommand, SubmitCommand, TriggerCommand,
        WorkflowCommand,
    },
    command::service::types::WorkflowSetSubmitAggregatorResult,
    context::CliContext,
    service_json::{
        validate_block_interval_config, validate_block_interval_config_on_chain,
        validate_cron_config, ServiceJsonExt,
    },
};

/// Handle service commands - this function will be called from main.rs
pub async fn handle_service_command(
    ctx: &CliContext,
    file: PathBuf,
    json: bool,
    command: ServiceCommand,
) -> Result<()> {
    match command {
        ServiceCommand::Init { name } => {
            let result = init_service(&file, name)?;
            display_result(ctx, result, json)?;
        }
        ServiceCommand::Workflow { command } => match command {
            WorkflowCommand::Add { id } => {
                let result = add_workflow(&file, id)?;
                display_result(ctx, result, json)?;
            }
            WorkflowCommand::Delete { id } => {
                let result = delete_workflow(&file, id)?;
                display_result(ctx, result, json)?;
            }
            WorkflowCommand::Component { id, command } => {
                let result =
                    update_workflow_component(&ctx.config.ipfs_gateway, &file, id, command).await?;
                display_result(ctx, result, json)?;
            }
            WorkflowCommand::Trigger { id, command } => match command {
                TriggerCommand::SetCosmos {
                    address,
                    chain,
                    event_type,
                } => {
                    let query_client = ctx.new_cosmos_client(chain.id.clone()).await?.querier;
                    let result =
                        set_cosmos_trigger(query_client, &file, id, address, chain, event_type)?;
                    display_result(ctx, result, json)?;
                }
                TriggerCommand::SetEvm {
                    address,
                    chain,
                    event_hash,
                } => {
                    let result = set_evm_trigger(&file, id, address, chain, event_hash)?;
                    display_result(ctx, result, json)?;
                }
                TriggerCommand::SetBlockInterval {
                    chain,
                    n_blocks,
                    start_block,
                    end_block,
                } => {
                    let result = set_block_interval_trigger(
                        &file,
                        id,
                        chain,
                        n_blocks,
                        start_block,
                        end_block,
                    )?;
                    display_result(ctx, result, json)?;
                }
                TriggerCommand::SetCron {
                    schedule,
                    start_time,
                    end_time,
                } => {
                    let result = set_cron_trigger(&file, id, schedule, start_time, end_time)?;
                    display_result(ctx, result, json)?;
                }
                TriggerCommand::SetAtProtocol {
                    collection,
                    repo_did,
                    action,
                } => {
                    let result = set_atproto_trigger(&file, id, collection, repo_did, action)?;
                    display_result(ctx, result, json)?;
                }
                TriggerCommand::SetHypercoreAppend { feed_key } => {
                    let result = set_hypercore_append_trigger(&file, id, feed_key)?;
                    display_result(ctx, result, json)?;
                }
            },
            WorkflowCommand::Submit { id, command } => match command {
                SubmitCommand::SetAggregator {} => {
                    let result = set_aggregator_submit(&file, id)?;
                    display_result(ctx, result, json)?;
                }
                SubmitCommand::SetNone {} => {
                    let result = set_none_submit(&file, id)?;
                    display_result(ctx, result, json)?;
                }
                SubmitCommand::Component { component } => {
                    let result =
                        modify_aggregator_component(&ctx.config.ipfs_gateway, &file, id, component)
                            .await?;
                    display_result(ctx, result, json)?;
                }
            },
        },
        ServiceCommand::Manager { command } => match command {
            ManagerCommand::SetEvm { chain, address } => {
                let result = set_evm_manager(&file, address, chain)?;
                display_result(ctx, result, json)?;
            }
        },
        ServiceCommand::UpdateStatus { status } => {
            let result = update_status(&file, status)?;
            display_result(ctx, result, json)?;
        }
        ServiceCommand::Validate {} => {
            let result = validate_service(&file, Some(ctx)).await?;
            display_result(ctx, result, json)?;
        }
    }

    Ok(())
}

// Helper function to handle display based on json flag
fn display_result<T: std::fmt::Display + Serialize>(
    ctx: &CliContext,
    result: T,
    json: bool,
) -> Result<()> {
    if json {
        print_result_as_json(result)?;
    } else {
        ctx.handle_display_result(result);
    }
    Ok(())
}

/// Helper function to print file content as JSON
fn print_result_as_json<T: Serialize>(result: T) -> Result<()> {
    // Print the pretty-printed JSON
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

/// Parse configuration from a JSON file containing flat key-value pairs
fn parse_config_from_file<P: AsRef<Path>>(config_file: P) -> Result<BTreeMap<String, String>> {
    let config_file = config_file.as_ref();

    // Read the config file
    let config_content = std::fs::read_to_string(config_file)
        .with_context(|| format!("Failed to read config file: {}", config_file.display()))?;

    // Parse the JSON
    let config_value: serde_json::Value =
        serde_json::from_str(&config_content).with_context(|| {
            format!(
                "Failed to parse JSON in config file: {}",
                config_file.display()
            )
        })?;

    // Ensure it's a flat object (no nested objects/arrays)
    let config_obj = match config_value {
        serde_json::Value::Object(obj) => obj,
        _ => {
            return Err(anyhow!(
                "Config file must contain a JSON object with key-value pairs"
            ))
        }
    };

    let mut config_map = BTreeMap::new();

    for (key, value) in config_obj {
        // Only allow simple string values (no nested objects or arrays)
        match value {
            serde_json::Value::String(s) => {
                config_map.insert(key, s);
            }
            serde_json::Value::Number(n) => {
                config_map.insert(key, n.to_string());
            }
            serde_json::Value::Bool(b) => {
                config_map.insert(key, b.to_string());
            }
            serde_json::Value::Null => {
                return Err(anyhow!(
                    "Config key '{}' has null value. All keys must have string values",
                    key
                ));
            }
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                // Serialize complex objects/arrays as JSON strings
                config_map.insert(key, value.to_string());
            }
        }
    }

    Ok(config_map)
}

/// Helper function to load a service, modify it, and save it back
pub fn modify_service_file<P, F, R>(file_path: P, modifier: F) -> Result<R>
where
    P: AsRef<Path>,
    F: FnOnce(ServiceBuilder) -> Result<(ServiceBuilder, R)>,
{
    let file_path = file_path.as_ref();

    // Read the service file
    let service_json = std::fs::read_to_string(file_path)?;

    // Parse the service JSON
    let service: ServiceBuilder = serde_json::from_str(&service_json)?;

    // Apply the modification and get the result
    let (updated_service, result) = modifier(service)?;

    // Convert updated service to JSON
    let updated_service_json = serde_json::to_string_pretty(&updated_service)?;

    // Write the updated JSON back to file
    let mut file = File::create(file_path)?;
    file.write_all(updated_service_json.as_bytes())?;

    Ok(result)
}

enum ComponentTarget<'a> {
    Direct(&'a mut Component),
    Json(&'a mut ComponentBuilder),
}

/// Helper to get mutable Component reference, handling Submit::Aggregator case separately
fn get_target_component<'a>(
    workflow: &'a mut WorkflowBuilder,
    context: &ComponentContext,
) -> Result<ComponentTarget<'a>> {
    match context {
        ComponentContext::Workflow { .. } => Ok(ComponentTarget::Json(&mut workflow.component)),
        ComponentContext::Aggregator { .. } => match &mut workflow.submit {
            SubmitBuilder::Submit(Submit::Aggregator { component, .. }) => Ok(ComponentTarget::Direct(component)),
            SubmitBuilder::AggregatorBuilder(AggregatorBuilder::Aggregator { component, .. }) => Ok(ComponentTarget::Json(component)),
            _ => anyhow::bail!("Cannot modify aggregator component when the workflow's submit is not set to aggregator"),
        },
    }
}

fn get_component_from_target(target: ComponentTarget<'_>) -> Result<&mut Component> {
    match target {
        ComponentTarget::Direct(component) => Ok(component),
        ComponentTarget::Json(component_json) => component_json
            .as_component_mut()
            .ok_or_else(|| anyhow::anyhow!("Component is unset. Set the component source first.")),
    }
}

fn build_component_result(
    component: &Component,
    context: &ComponentContext,
    command: &ComponentCommand,
    file_path: &Path,
) -> Result<ComponentOperationResult> {
    let result = match command {
        ComponentCommand::Permissions { .. } => ComponentOperationResult::Permissions {
            context: context.clone(),
            permissions: component.permissions.clone(),
            file_path: file_path.to_path_buf(),
        },
        ComponentCommand::FuelLimit { .. } => ComponentOperationResult::FuelLimit {
            context: context.clone(),
            fuel_limit: component.fuel_limit,
            file_path: file_path.to_path_buf(),
        },
        ComponentCommand::Config { .. } => ComponentOperationResult::Config {
            context: context.clone(),
            config: component.config.clone(),
            file_path: file_path.to_path_buf(),
        },
        ComponentCommand::TimeLimit { .. } => ComponentOperationResult::TimeLimit {
            context: context.clone(),
            time_limit_seconds: component.time_limit_seconds,
            file_path: file_path.to_path_buf(),
        },
        ComponentCommand::Env { .. } => ComponentOperationResult::EnvKeys {
            context: context.clone(),
            env_keys: component.env_keys.clone(),
            file_path: file_path.to_path_buf(),
        },
        ComponentCommand::SetSourceDigest { .. }
        | ComponentCommand::SetSourceRegistry { .. }
        | ComponentCommand::SetSourceUri { .. } => {
            unreachable!("Source commands should be handled separately")
        }
    };
    Ok(result)
}

/// Unified component operation handler for both workflow and aggregator components
pub async fn update_component(
    ipfs_gateway: &str,
    file_path: &Path,
    workflow_id: WorkflowId,
    context: ComponentContext,
    command: ComponentCommand,
) -> Result<ComponentOperationResult> {
    // Handle async command separately for use in modify_service_file
    match &command {
        ComponentCommand::SetSourceRegistry {
            domain,
            package,
            version,
        } => {
            let resolved_domain = domain.clone().unwrap_or("wa.dev".to_string());
            let wkg_client = WkgClient::new(resolved_domain.clone())?;
            let (digest, resolved_version) = wkg_client
                .get_digest(domain.clone(), package, version.as_ref())
                .await?;

            let registry = Registry {
                digest: digest.clone(),
                domain: domain.clone(),
                version: version.clone(),
                package: package.clone(),
            };

            modify_service_file(file_path, |mut service| {
                let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
                    anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
                })?;

                match get_target_component(workflow, &context)? {
                    ComponentTarget::Direct(component) => {
                        component.source = ComponentSource::Registry { registry };
                    }
                    ComponentTarget::Json(component_json) => {
                        let source = ComponentSource::Registry { registry };
                        let new_component = Component::new(source);
                        *component_json = ComponentBuilder::Component(new_component);
                    }
                }

                Ok((service, ()))
            })?;

            Ok(ComponentOperationResult::SourceRegistry {
                context,
                domain: resolved_domain,
                package: package.clone(),
                digest,
                version: resolved_version,
                file_path: file_path.to_path_buf(),
            })
        }

        ComponentCommand::SetSourceUri { uri } => {
            let bytes = fetch_bytes(uri, ipfs_gateway).await?;
            let digest = ComponentDigest::hash(&bytes);

            modify_service_file(file_path, |mut service| {
                let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
                    anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
                })?;

                match get_target_component(workflow, &context)? {
                    ComponentTarget::Direct(component) => {
                        component.source = ComponentSource::Download {
                            uri: uri.clone(),
                            digest: digest.clone(),
                        };
                    }
                    ComponentTarget::Json(component_json) => {
                        let source = ComponentSource::Download {
                            uri: uri.clone(),
                            digest: digest.clone(),
                        };
                        let new_component = Component::new(source);
                        *component_json = ComponentBuilder::Component(new_component);
                    }
                }

                Ok((service, ()))
            })?;

            Ok(ComponentOperationResult::SourceUrl {
                context,
                uri: uri.to_string(),
                digest,
                file_path: file_path.to_path_buf(),
            })
        }
        _ => modify_service_file(file_path, |mut service| {
            let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
                anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
            })?;

            let target = get_target_component(workflow, &context)?;
            let result = execute_sync_command(target, &command, &context, file_path)?;

            Ok((service, result))
        }),
    }
}

/// Execute synchronous component commands
fn execute_sync_command(
    target: ComponentTarget<'_>,
    command: &ComponentCommand,
    context: &ComponentContext,
    file_path: &Path,
) -> Result<ComponentOperationResult> {
    match command {
        ComponentCommand::SetSourceDigest { digest } => {
            // Handle source setting directly
            match target {
                ComponentTarget::Direct(component) => {
                    component.source = ComponentSource::Digest(digest.clone());
                }
                ComponentTarget::Json(component_json) => {
                    if component_json.is_unset() {
                        let new_component = Component::new(ComponentSource::Digest(digest.clone()));
                        *component_json = ComponentBuilder::new(new_component);
                    } else if let Some(component) = component_json.as_component_mut() {
                        component.source = ComponentSource::Digest(digest.clone());
                    }
                }
            }
            Ok(ComponentOperationResult::SourceDigest {
                context: context.clone(),
                digest: digest.clone(),
                file_path: file_path.to_path_buf(),
            })
        }
        other_command => {
            let component = get_component_from_target(target)?;
            apply_component_command(component, other_command.clone())?;
            build_component_result(component, context, other_command, file_path)
        }
    }
}

/// Apply a component command to a mutable component reference
fn apply_component_command(component: &mut Component, command: ComponentCommand) -> Result<()> {
    match command {
        ComponentCommand::SetSourceDigest { .. }
        | ComponentCommand::SetSourceRegistry { .. }
        | ComponentCommand::SetSourceUri { .. } => {
            unreachable!("This should be handled in caller")
        }
        ComponentCommand::Permissions {
            http_hosts,
            file_system,
        } => {
            if let Some(mut hosts) = http_hosts {
                hosts = hosts
                    .into_iter()
                    .map(|host| host.trim().to_string())
                    .filter(|host| !host.is_empty())
                    .collect();

                component.permissions.allowed_http_hosts = if hosts.is_empty() {
                    AllowedHostPermission::None
                } else if hosts.len() == 1 && hosts[0] == "*" {
                    AllowedHostPermission::All
                } else {
                    AllowedHostPermission::Only(hosts)
                };
            }
            if let Some(fs_perm) = file_system {
                component.permissions.file_system = fs_perm;
            }
        }
        ComponentCommand::FuelLimit { fuel } => {
            component.fuel_limit = fuel;
        }
        ComponentCommand::TimeLimit { seconds } => {
            component.time_limit_seconds = seconds;
        }
        ComponentCommand::Config {
            values,
            config_file,
        } => {
            if let Some(config_file) = config_file {
                // Load config from JSON file
                let config_map = parse_config_from_file(config_file)?;
                component.config = config_map;
            } else if let Some(values) = values {
                // Parse key=value pairs from command line
                let mut config_pairs = BTreeMap::new();
                for value in values {
                    match value.split_once('=') {
                        Some((key, value)) => {
                            let key = key.trim().to_string();
                            let value = value.trim().to_string();
                            if key.is_empty() {
                                return Err(anyhow!("Empty key in config value: '{}'", value));
                            }
                            config_pairs.insert(key, value);
                        }
                        None => {
                            return Err(anyhow!(
                                "Invalid config format: '{}'. Expected 'key=value'",
                                value
                            ));
                        }
                    }
                }
                component.config = config_pairs;
            } else {
                // Clear all config values
                component.config.clear();
            }
        }
        ComponentCommand::Env { values } => {
            if let Some(values) = values {
                let mut validated_env_keys = BTreeSet::new();
                for key in values {
                    let key = key.trim().to_string();
                    if key.is_empty() {
                        continue;
                    }
                    if !key.starts_with(WAVS_ENV_PREFIX) {
                        return Err(anyhow!(
                            "Environment variable '{}' must start with '{}'",
                            key,
                            WAVS_ENV_PREFIX
                        ));
                    }
                    validated_env_keys.insert(key);
                }
                component.env_keys = validated_env_keys;
            } else {
                component.env_keys.clear();
            }
        }
    }
    Ok(())
}

/// Run the service initialization
pub fn init_service(file_path: &Path, name: String) -> Result<ServiceInitResult> {
    // Create the service
    let service = ServiceBuilder {
        name,
        workflows: BTreeMap::new(),
        status: ServiceStatus::Active,
        manager: ServiceManagerBuilder::default(),
    };

    // Convert service to JSON
    let service_json = serde_json::to_string_pretty(&service)?;

    // Create the directory if it doesn't exist
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Write the JSON to file
    let mut file = File::create(file_path)?;
    file.write_all(service_json.as_bytes())?;

    Ok(ServiceInitResult {
        service,
        file_path: file_path.to_path_buf(),
    })
}

/// Add a workflow to a service
pub fn add_workflow(file_path: &Path, id: Option<WorkflowId>) -> Result<WorkflowAddResult> {
    modify_service_file(file_path, |mut service| {
        // Generate workflow ID if not provided
        let workflow_id = match id {
            Some(id) => id,
            None => WorkflowId::new(Uuid::now_v7().as_hyphenated().to_string())?,
        };

        // Create default trigger, component, and submit
        let trigger = TriggerBuilder::default();
        let component = ComponentBuilder::default();
        let submit = SubmitBuilder::default();

        // Create a new workflow entry
        let workflow = WorkflowBuilder {
            trigger,
            component,
            submit,
        };

        // Add the workflow to the service
        service.workflows.insert(workflow_id.clone(), workflow);

        Ok((
            service,
            WorkflowAddResult {
                workflow_id,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Delete a workflow from a service
pub fn delete_workflow(file_path: &Path, workflow_id: WorkflowId) -> Result<WorkflowDeleteResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        if !service.workflows.contains_key(&workflow_id) {
            return Err(anyhow::anyhow!(
                "Workflow with ID '{}' not found in service",
                workflow_id
            ));
        }

        // Remove the workflow
        service.workflows.remove(&workflow_id);

        Ok((
            service,
            WorkflowDeleteResult {
                workflow_id,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set a Cosmos contract event trigger for a workflow
pub fn set_cosmos_trigger(
    query_client: CosmosQueryClient,
    file_path: &Path,
    workflow_id: WorkflowId,
    address_str: String,
    chain: ChainKey,
    event_type: String,
) -> Result<WorkflowTriggerResult> {
    // Parse the Cosmos address
    let address = query_client
        .chain_config
        .parse_address(&address_str)?
        .try_into()?;

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the trigger
        let trigger = Trigger::CosmosContractEvent {
            address,
            chain,
            event_type,
        };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set an EVM contract event trigger for a workflow
pub fn set_evm_trigger(
    file_path: &Path,
    workflow_id: WorkflowId,
    address: alloy_primitives::Address,
    chain: ChainKey,
    event_hash_str: String,
) -> Result<WorkflowTriggerResult> {
    // Order the match cases from most explicit to event parsing:
    // 1. 0x-prefixed hex string
    // 2. raw hex string (no 0x)
    // 3. event name to be parsed into signature
    let trigger_event_name = match event_hash_str {
        name if name.starts_with("0x") => name,
        name if const_hex::const_check(name.as_bytes()).is_ok() => name,
        name => Event::parse(&name)
            .context("Invalid event signature format")?
            .selector()
            .to_string(),
    };

    let mut event_hash: [u8; 32] = [0; 32];
    event_hash.copy_from_slice(&const_hex::decode(trigger_event_name)?);

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the trigger
        let trigger = Trigger::EvmContractEvent {
            address,
            chain,
            event_hash: ByteArray::new(event_hash),
        };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

pub fn set_block_interval_trigger(
    file_path: &Path,
    workflow_id: WorkflowId,
    chain: ChainKey,
    n_blocks: NonZeroU32,
    start_block: Option<NonZeroU64>,
    end_block: Option<NonZeroU64>,
) -> Result<WorkflowTriggerResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        validate_block_interval_config(start_block, end_block).map_err(|e| anyhow!(e))?;

        let trigger = Trigger::BlockInterval {
            chain,
            n_blocks,
            start_block,
            end_block,
        };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

pub fn set_cron_trigger(
    file_path: &Path,
    workflow_id: WorkflowId,
    schedule: cron::Schedule,
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
) -> Result<WorkflowTriggerResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        validate_cron_config(start_time, end_time).map_err(|e| anyhow!(e))?;

        let trigger = Trigger::Cron {
            schedule: schedule.to_string(),
            start_time,
            end_time,
        };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set an ATProto Jetstream event trigger for a workflow
pub fn set_atproto_trigger(
    file_path: &Path,
    workflow_id: WorkflowId,
    collection: String,
    repo_did: Option<String>,
    action: Option<AtProtoAction>,
) -> Result<WorkflowTriggerResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Validate collection format (basic NSID validation)
        if !collection.contains('.') {
            return Err(anyhow!(
                "Invalid collection format '{}'. Expected NSID format like 'app.bsky.feed.post'",
                collection
            ));
        }

        // Validate DID format if provided
        if let Some(ref did) = repo_did {
            if !did.starts_with("did:") {
                return Err(anyhow!(
                    "Invalid DID format '{}'. Must start with 'did:'",
                    did
                ));
            }
        }

        let trigger = Trigger::AtProtoEvent {
            collection,
            repo_did,
            action,
        };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set a Hypercore append trigger for a workflow
pub fn set_hypercore_append_trigger(
    file_path: &Path,
    workflow_id: WorkflowId,
    feed_key: String,
) -> Result<WorkflowTriggerResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;
        if feed_key.trim().is_empty() {
            return Err(anyhow::anyhow!("Hypercore feed key cannot be empty"));
        }

        let trigger = Trigger::HypercoreAppend { feed_key };
        workflow.trigger = TriggerBuilder::Trigger(trigger.clone());

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Update workflow component using unified logic
pub async fn update_workflow_component(
    ipfs_gateway: &str,
    file_path: &Path,
    workflow_id: WorkflowId,
    command: ComponentCommand,
) -> Result<ComponentOperationResult> {
    use crate::command::service::types::ComponentContext;

    let context = ComponentContext::Workflow {
        workflow_id: workflow_id.clone(),
    };
    update_component(ipfs_gateway, file_path, workflow_id, context, command).await
}

/// Set an EVM manager for the service
pub fn set_evm_manager(
    file_path: &Path,
    address: alloy_primitives::Address,
    chain: ChainKey,
) -> Result<EvmManagerResult> {
    modify_service_file(file_path, |mut service| {
        service.manager = ServiceManagerBuilder::Manager(ServiceManager::Evm {
            chain: chain.clone(),
            address,
        });

        Ok((
            service,
            EvmManagerResult {
                chain,
                address,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

fn update_status(file_path: &PathBuf, status: ServiceStatus) -> Result<UpdateStatusResult> {
    modify_service_file(file_path, |mut service| {
        service.status = status;

        Ok((
            service,
            UpdateStatusResult {
                status,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Validate a service JSON file
pub async fn validate_service(
    file_path: &Path,
    ctx: Option<&CliContext>,
) -> Result<ServiceValidationResult> {
    // Read the service file
    let service_json = std::fs::read_to_string(file_path)?;

    // Parse the service JSON
    let service: ServiceBuilder = serde_json::from_str(&service_json)?;

    // Get basic validation errors from the ServiceJson::validate method
    let mut errors = service.validate();

    // All remaining validation needs CliContext, so only do it if ctx is provided
    if let Some(ctx) = ctx {
        let chains = {
            ctx.config
                .chains
                .read()
                .map_err(|_| anyhow!("Chains lock is poisoned"))?
                .clone()
        };
        validate_registry_availability(&ctx.config.wavs_endpoint, &mut errors).await;

        let mut chains_to_validate = HashSet::new();
        let mut triggers = Vec::new();
        let mut submits = Vec::new();

        for (workflow_id, workflow) in &service.workflows {
            if let TriggerBuilder::Trigger(trigger) = &workflow.trigger {
                match trigger {
                    Trigger::CosmosContractEvent { chain, .. } => {
                        chains_to_validate.insert((chain.clone(), ChainType::Cosmos));

                        if let Ok(client) = ctx.new_cosmos_client(chain.id.clone()).await {
                            validate_workflow_trigger(
                                workflow_id,
                                trigger,
                                &client.querier,
                                &mut errors,
                            )
                            .await;
                        } else {
                            errors.push(format!(
                                "Workflow '{}' uses chain '{}' in Cosmos trigger,
  but client configuration is invalid",
                                workflow_id, chain
                            ));
                        }
                    }
                    Trigger::EvmContractEvent { chain, .. } => {
                        chains_to_validate.insert((chain.clone(), ChainType::EVM));
                    }
                    Trigger::BlockInterval {
                        chain,
                        start_block,
                        end_block,
                        ..
                    } => match chains.get_chain(chain) {
                        None => {
                            errors.push(format!(
                                "Workflow '{}' uses chain '{}' in BlockInterval
   trigger, but chain is missing",
                                workflow_id, chain
                            ));
                        }
                        Some(AnyChainConfig::Cosmos(_)) => {
                            let cosmos_client = ctx.new_cosmos_client(chain.id.clone()).await?;
                            let block_height = cosmos_client.querier.block_height().await?;
                            if let Err(err) = validate_block_interval_config_on_chain(
                                *start_block,
                                *end_block,
                                block_height,
                            ) {
                                errors.push(format!(
                                    "Workflow '{}' has invalid block interval
  configuration: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                        Some(AnyChainConfig::Evm(_)) => {
                            let evm_client = ctx.new_evm_client_read_only(chain.id.clone()).await?;
                            let block_height = evm_client.provider.get_block_number().await?;
                            if let Err(err) = validate_block_interval_config_on_chain(
                                *start_block,
                                *end_block,
                                block_height,
                            ) {
                                errors.push(format!(
                                    "Workflow '{}' has invalid block interval
  configuration: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    },
                    _ => {}
                }

                triggers.push((workflow_id, trigger));
            }

            if let SubmitBuilder::Submit(submit) = &workflow.submit {
                submits.push((workflow_id, submit));
            }
        }

        let service_manager =
            if let ServiceManagerBuilder::Manager(service_manager) = &service.manager {
                match service_manager {
                    ServiceManager::Evm { chain, .. } => {
                        chains_to_validate.insert((chain.clone(), ChainType::EVM));
                    }
                    ServiceManager::Cosmos { chain, .. } => {
                        chains_to_validate.insert((chain.clone(), ChainType::Cosmos));
                    }
                }

                Some(service_manager)
            } else {
                None
            };

        // Build maps of clients for chains actually used
        let mut cosmos_clients = HashMap::new();
        let mut evm_providers = HashMap::new();

        // Only get clients for chains actually used in triggers or submits
        for (chain, chain_type) in chains_to_validate.iter() {
            match chain_type {
                ChainType::Cosmos => {
                    if let Ok(client) = ctx.new_cosmos_client(chain.id.clone()).await {
                        cosmos_clients.insert(chain.clone(), client.querier);
                    }
                }
                ChainType::EVM => {
                    if let Ok(client) = ctx.new_evm_client_read_only(chain.id.clone()).await {
                        evm_providers.insert(chain.clone(), client.provider.root().clone());
                    }
                }
            }
        }

        // Validate that referenced contracts exist on-chain
        if !cosmos_clients.is_empty() || !evm_providers.is_empty() {
            if let Err(err) = validate_contracts_exist(
                &service.name,
                triggers,
                service_manager,
                &evm_providers,
                &cosmos_clients,
                &mut errors,
            )
            .await
            {
                errors.push(format!("Error during contract validation: {}", err));
            }
        }
    }

    Ok(ServiceValidationResult {
        service_name: service.name,
        errors,
    })
}

/// Set an Aggregator submit for a workflow
pub fn set_aggregator_submit(
    file_path: &Path,
    workflow_id: WorkflowId,
) -> Result<WorkflowSetSubmitAggregatorResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        workflow.submit = SubmitBuilder::AggregatorBuilder(AggregatorBuilder::Aggregator {
            component: ComponentBuilder::new_unset(),
            signature_kind: SignatureKind::evm_default(),
        });

        Ok((
            service,
            WorkflowSetSubmitAggregatorResult {
                workflow_id,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set the submit to None for a workflow
pub fn set_none_submit(
    file_path: &Path,
    workflow_id: WorkflowId,
) -> Result<WorkflowSetSubmitNoneResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        let submit = Submit::None;
        workflow.submit = SubmitBuilder::Submit(submit);

        Ok((
            service,
            WorkflowSetSubmitNoneResult {
                workflow_id,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Modify an aggregator component using unified logic
pub async fn modify_aggregator_component(
    ipfs_gateway: &str,
    file_path: &Path,
    workflow_id: WorkflowId,
    component_cmd: ComponentCommand,
) -> Result<ComponentOperationResult> {
    let context = ComponentContext::Aggregator {
        workflow_id: workflow_id.clone(),
    };
    update_component(ipfs_gateway, file_path, workflow_id, context, component_cmd).await
}
