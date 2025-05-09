use alloy_json_abi::Event;
use alloy_provider::{Provider, RootProvider};
use anyhow::{anyhow, Context as _, Result};
use layer_climb::{
    prelude::{Address, ConfigAddressExt as _},
    querier::QueryClient as CosmosQueryClient,
};
use reqwest::Client;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs::File,
    io::Write,
    num::NonZeroU32,
    path::{Path, PathBuf},
};
use utils::{config::WAVS_ENV_PREFIX, wkg::WkgClient};
use uuid::Uuid;
use wasm_pkg_client::{PackageRef, Version};
use wavs_types::{
    Aggregator, AllowedHostPermission, ByteArray, ChainName, Component, ComponentSource, Digest,
    EvmContractSubmission, Permissions, Registry, ServiceID, ServiceManager, ServiceStatus, Submit,
    Timestamp, Trigger, WorkflowID,
};

use crate::{
    args::{
        ComponentCommand, ManagerCommand, ServiceCommand, SubmitCommand, TriggerCommand,
        WorkflowCommand,
    },
    context::CliContext,
    service_json::{
        validate_cron_config, ComponentJson, ServiceJson, ServiceManagerJson, SubmitJson,
        TriggerJson, WorkflowJson,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChainType {
    Cosmos,
    EVM,
}

/// Handle service commands - this function will be called from main.rs
pub async fn handle_service_command(
    ctx: &CliContext,
    file: PathBuf,
    json: bool,
    command: ServiceCommand,
) -> Result<()> {
    match command {
        ServiceCommand::Init { name, id } => {
            let result = init_service(&file, name, id)?;
            display_result(ctx, result, &file, json)?;
        }
        ServiceCommand::Workflow { command } => match command {
            WorkflowCommand::Add { id } => {
                let result = add_workflow(&file, id)?;
                display_result(ctx, result, &file, json)?;
            }
            WorkflowCommand::Delete { id } => {
                let result = delete_workflow(&file, id)?;
                display_result(ctx, result, &file, json)?;
            }
            WorkflowCommand::Component { id, command } => match command {
                ComponentCommand::SetSourceDigest { digest } => {
                    let result = set_component_source_digest(&file, id, digest)?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::SetSourceRegistry {
                    domain,
                    package,
                    version,
                } => {
                    let result = set_component_source_registry(
                        ctx.config.registry_domain.clone(),
                        &file,
                        id,
                        domain,
                        package,
                        version,
                    )
                    .await?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::Permissions {
                    http_hosts,
                    file_system,
                } => {
                    let result = update_component_permissions(&file, id, http_hosts, file_system)?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::FuelLimit { fuel } => {
                    let result = update_component_fuel_limit(&file, id, fuel)?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::Config { values } => {
                    let result = update_component_config(&file, id, values)?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::TimeLimit { seconds } => {
                    let result = update_component_time_limit_seconds(&file, id, seconds)?;
                    display_result(ctx, result, &file, json)?;
                }
                ComponentCommand::Env { values } => {
                    let result = update_component_env_keys(&file, id, values)?;
                    display_result(ctx, result, &file, json)?;
                }
            },
            WorkflowCommand::Submit { id, command } => match command {
                SubmitCommand::SetEvm {
                    address,
                    chain_name,
                    max_gas,
                } => {
                    let result = set_evm_submit(&file, id, address, chain_name, max_gas)?;
                    display_result(ctx, result, &file, json)?;
                }
                SubmitCommand::SetAggregator {
                    url,
                    chain_name,
                    address,
                    max_gas,
                } => {
                    let result =
                        set_aggregator_submit(&file, id, url, chain_name, address, max_gas)?;
                    display_result(ctx, result, &file, json)?;
                }
            },
            WorkflowCommand::Trigger { id, command } => match command {
                TriggerCommand::SetCosmos {
                    address,
                    chain_name,
                    event_type,
                } => {
                    let query_client = ctx.new_cosmos_client(&chain_name).await?.querier;
                    let result = set_cosmos_trigger(
                        query_client,
                        &file,
                        id,
                        address,
                        chain_name,
                        event_type,
                    )?;
                    display_result(ctx, result, &file, json)?;
                }
                TriggerCommand::SetEvm {
                    address,
                    chain_name,
                    event_hash,
                } => {
                    let result = set_evm_trigger(&file, id, address, chain_name, event_hash)?;
                    display_result(ctx, result, &file, json)?;
                }
                TriggerCommand::SetBlockInterval {
                    chain_name,
                    n_blocks,
                } => {
                    let result = set_block_interval_trigger(&file, id, chain_name, n_blocks)?;
                    display_result(ctx, result, &file, json)?;
                }
                TriggerCommand::SetCron {
                    schedule,
                    start_time,
                    end_time,
                } => {
                    let result = set_cron_trigger(&file, id, schedule, start_time, end_time)?;
                    display_result(ctx, result, &file, json)?;
                }
            },
        },
        ServiceCommand::Manager { command } => match command {
            ManagerCommand::SetEvm {
                chain_name,
                address,
            } => {
                let result = set_evm_manager(&file, address, chain_name)?;
                display_result(ctx, result, &file, json)?;
            }
        },
        ServiceCommand::Validate {} => {
            let result = validate_service(&file, Some(ctx)).await?;
            display_result(ctx, result, &file, json)?;
        }
    }

    Ok(())
}

// Helper function to handle display based on json flag
fn display_result<T: std::fmt::Display>(
    ctx: &CliContext,
    result: T,
    file_path: &Path,
    json: bool,
) -> Result<()> {
    if json {
        print_file_as_json(file_path)?;
    } else {
        ctx.handle_display_result(result);
    }
    Ok(())
}

/// Helper function to print file content as JSON
fn print_file_as_json(file_path: &Path) -> Result<()> {
    // Read the file content
    let file_content = std::fs::read_to_string(file_path)?;

    // Parse it as JSON to ensure it's valid
    let json_value: serde_json::Value = serde_json::from_str(&file_content)?;

    // Print the pretty-printed JSON
    println!("{}", serde_json::to_string_pretty(&json_value)?);

    Ok(())
}

/// Result of service initialization
#[derive(Debug, Clone)]
pub struct ServiceInitResult {
    /// The generated service
    pub service: ServiceJson,
    /// The file path where the service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ServiceInitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Service JSON generated successfully!")?;
        writeln!(f, "  ID:   {}", self.service.id)?;
        writeln!(f, "  Name: {}", self.service.name)?;
        writeln!(f, "  File: {}", self.file_path.display())
    }
}

/// Result of setting a component's source to a digest
#[derive(Debug, Clone)]
pub struct ComponentSourceDigestResult {
    /// The component digest
    pub digest: Digest,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentSourceDigestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component source set to digest successfully!")?;
        writeln!(f, "  Digest:       {}", self.digest)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of setting a component's source to a registry
#[derive(Debug, Clone)]
pub struct ComponentSourceRegistryResult {
    /// The domain
    pub domain: String,
    /// The package reference
    pub package: PackageRef,
    /// The component digest (retrieved from registry)
    pub digest: Digest,
    /// The version
    pub version: Version,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentSourceRegistryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component source set to registry package successfully!")?;
        writeln!(f, "  Domain:       {}", self.domain)?;
        writeln!(f, "  Package:      {}", self.package)?;
        writeln!(f, "  Version:      {}", self.version)?;
        writeln!(f, "  Digest:       {}", self.digest)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of updating a component's environment variables
#[derive(Debug, Clone)]
pub struct ComponentEnvKeysResult {
    /// The updated environment variable keys
    pub env_keys: BTreeSet<String>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentEnvKeysResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component environment variables updated successfully!")?;
        if self.env_keys.is_empty() {
            writeln!(f, "  Env Keys:    No environment variables")?;
        } else {
            writeln!(f, "  Env Keys:")?;
            for key in &self.env_keys {
                writeln!(f, "    {}", key)?;
            }
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of adding a workflow
#[derive(Debug, Clone)]
pub struct WorkflowAddResult {
    /// The workflow id
    pub workflow_id: WorkflowID,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowAddResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow added successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of deleting a workflow
#[derive(Debug, Clone)]
pub struct WorkflowDeleteResult {
    /// The workflow id that was deleted
    pub workflow_id: WorkflowID,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowDeleteResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow deleted successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a workflow's trigger
#[derive(Debug, Clone)]
pub struct WorkflowTriggerResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowID,
    /// The updated trigger type
    pub trigger: Trigger,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowTriggerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow trigger updated successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;

        match &self.trigger {
            Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } => {
                writeln!(f, "  Trigger Type: Cosmos Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                writeln!(f, "    Event Type: {}", event_type)?;
            }
            Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } => {
                writeln!(f, "  Trigger Type: EVM Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                writeln!(f, "    Event Hash: {}", event_hash)?;
            }
            Trigger::Manual => {
                writeln!(f, "  Trigger Type: Manual")?;
            }
            Trigger::BlockInterval {
                chain_name,
                n_blocks,
            } => {
                writeln!(f, "  Trigger Type: Block Interval")?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                writeln!(f, "    Interval:   {} blocks", n_blocks)?;
            }
            Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => {
                writeln!(f, "  Trigger Type: Cron")?;
                writeln!(f, "    Schedule:   {}", schedule)?;
                if let Some(start) = start_time {
                    writeln!(f, "    Start Time: {}", start.as_nanos())?;
                } else {
                    writeln!(f, "    Start Time: None")?;
                }
                if let Some(end) = end_time {
                    writeln!(f, "    End Time:   {}", end.as_nanos())?;
                } else {
                    writeln!(f, "    End Time:   None")?;
                }
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a workflow's submit
#[derive(Debug, Clone)]
pub struct WorkflowSubmitResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowID,
    /// The updated submit type
    pub submit: Submit,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowSubmitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow submit updated successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;

        match &self.submit {
            Submit::EvmContract(EvmContractSubmission {
                address,
                chain_name,
                max_gas,
            }) => {
                writeln!(f, "  Submit Type: EVM Service Handler")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                if let Some(gas) = max_gas {
                    writeln!(f, "    Max Gas:    {}", gas)?;
                }
            }
            Submit::None => {
                writeln!(f, "  Submit Type: None")?;
            }
            Submit::Aggregator { url } => {
                writeln!(f, "  Submit Type: Aggregator")?;
                writeln!(f, "    Url:    {}", url)?;
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of setting the EVM manager
#[derive(Debug, Clone)]
pub struct EvmManagerResult {
    /// The EVM chain name
    pub chain_name: ChainName,
    /// The EVM address
    pub address: alloy_primitives::Address,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for EvmManagerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "EVM manager set successfully!")?;
        writeln!(f, "  Address:      {}", self.address)?;
        writeln!(f, "  Chain:        {}", self.chain_name)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of service validation
#[derive(Debug, Clone)]
pub struct ServiceValidationResult {
    /// The service ID
    pub service_id: String,
    /// Any errors generated during validation
    pub errors: Vec<String>,
}

impl std::fmt::Display for ServiceValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.errors.is_empty() {
            writeln!(f, "✅ Service validation successful!")?;
            writeln!(f, "   Service ID: {}", self.service_id)?;
        } else {
            writeln!(f, "❌ Service validation failed with errors")?;
            writeln!(f, "   Service ID: {}", self.service_id)?;
            writeln!(f, "   Errors:")?;
            for (i, error) in self.errors.iter().enumerate() {
                writeln!(f, "   {}: {}", i + 1, error)?;
            }
        }
        Ok(())
    }
}

/// Helper function to load a service, modify it, and save it back
pub fn modify_service_file<P, F, R>(file_path: P, modifier: F) -> Result<R>
where
    P: AsRef<Path>,
    F: FnOnce(ServiceJson) -> Result<(ServiceJson, R)>,
{
    let file_path = file_path.as_ref();

    // Read the service file
    let service_json = std::fs::read_to_string(file_path)?;

    // Parse the service JSON
    let service: ServiceJson = serde_json::from_str(&service_json)?;

    // Apply the modification and get the result
    let (updated_service, result) = modifier(service)?;

    // Convert updated service to JSON
    let updated_service_json = serde_json::to_string_pretty(&updated_service)?;

    // Write the updated JSON back to file
    let mut file = File::create(file_path)?;
    file.write_all(updated_service_json.as_bytes())?;

    Ok(result)
}

/// Result of updating a component's fuel limit
#[derive(Debug, Clone)]
pub struct ComponentFuelLimitResult {
    /// The updated fuel limit
    pub fuel_limit: Option<u64>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentFuelLimitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component fuel limit updated successfully!")?;
        match self.fuel_limit {
            Some(limit) => writeln!(f, "  Fuel Limit:   {}", limit)?,
            None => writeln!(f, "  Fuel Limit:   No limit (removed)")?,
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a component's configuration
#[derive(Debug, Clone)]
pub struct ComponentConfigResult {
    /// The updated configuration
    pub config: BTreeMap<String, String>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentConfigResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component configuration updated successfully!")?;
        if self.config.is_empty() {
            writeln!(f, "  Config:      No configuration items")?;
        } else {
            writeln!(f, "  Config:")?;
            for (key, value) in &self.config {
                writeln!(f, "    {} => {}", key, value)?;
            }
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a component's maximum execution time
#[derive(Debug, Clone)]
pub struct ComponentTimeLimitResult {
    /// The updated maximum execution time in seconds
    pub time_limit_seconds: Option<u64>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentTimeLimitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component maximum execution time updated successfully!")?;
        match self.time_limit_seconds {
            Some(seconds) => writeln!(f, "  Max Execution Time: {} seconds", seconds)?,
            None => writeln!(f, "  Max Execution Time: Default (no explicit limit)")?,
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating component permissions
#[derive(Debug, Clone)]
pub struct ComponentPermissionsResult {
    /// The updated permissions
    pub permissions: Permissions,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentPermissionsResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component permissions updated successfully!")?;

        // Display HTTP permissions
        match &self.permissions.allowed_http_hosts {
            AllowedHostPermission::All => {
                writeln!(f, "  HTTP Hosts:   All allowed")?;
            }
            AllowedHostPermission::None => {
                writeln!(f, "  HTTP Hosts:   None allowed")?;
            }
            AllowedHostPermission::Only(hosts) => {
                writeln!(f, "  HTTP Hosts:   Only specific hosts allowed")?;
                for host in hosts {
                    writeln!(f, "    - {}", host)?;
                }
            }
        }

        // Display file system permission
        writeln!(
            f,
            "  File System: {}",
            if self.permissions.file_system {
                "Enabled"
            } else {
                "Disabled"
            }
        )?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Run the service initialization
pub fn init_service(
    file_path: &Path,
    name: String,
    id: Option<ServiceID>,
) -> Result<ServiceInitResult> {
    // Generate service ID if not provided
    let id = match id {
        Some(id) => id,
        None => ServiceID::new(Uuid::now_v7().as_hyphenated().to_string())?,
    };

    // Create the service
    let service = ServiceJson {
        id,
        name,
        workflows: BTreeMap::new(),
        status: ServiceStatus::Active,
        manager: ServiceManagerJson::default(),
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

/// Set the component source to a digest
pub fn set_component_source_digest(
    file_path: &Path,
    workflow_id: WorkflowID,
    digest: Digest,
) -> Result<ComponentSourceDigestResult> {
    modify_service_file(file_path, |mut service| {
        // Create a new component entry
        let component = Component::new(ComponentSource::Digest(digest.clone()));
        let component = ComponentJson::new(component);

        // Add the component to the service
        service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?
            .component = component;

        Ok((
            service,
            ComponentSourceDigestResult {
                digest,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set the component source to a registry package
pub async fn set_component_source_registry(
    default_domain: Option<String>,
    file_path: &Path,
    workflow_id: WorkflowID,
    domain: Option<String>,
    package: PackageRef,
    version: Option<Version>,
) -> Result<ComponentSourceRegistryResult> {
    let resolved_domain = domain.clone().unwrap_or(default_domain.unwrap());

    // Create a WkgClient using the registry domain
    let wkg_client = WkgClient::new(resolved_domain.clone())?;

    // Get the digest from the registry
    let (digest, resolved_version) = wkg_client
        .get_digest(domain.clone(), &package, version.as_ref())
        .await?;

    modify_service_file(file_path, |mut service| {
        // Get the workflow
        let workflow = service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?;

        // Create the Registry struct
        let registry = Registry {
            digest: digest.clone(),
            domain: domain.clone(),
            version,
            package: package.clone(),
        };

        // Create the component source
        let source = ComponentSource::Registry { registry };

        // Set the component in the workflow
        let component = Component::new(source);
        workflow.component = ComponentJson::Component(component);

        Ok((
            service,
            ComponentSourceRegistryResult {
                domain: resolved_domain,
                package,
                version: resolved_version,
                digest,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Add a workflow to a service
pub fn add_workflow(file_path: &Path, id: Option<WorkflowID>) -> Result<WorkflowAddResult> {
    modify_service_file(file_path, |mut service| {
        // Generate workflow ID if not provided
        let workflow_id = match id {
            Some(id) => id,
            None => WorkflowID::new(Uuid::now_v7().as_hyphenated().to_string())?,
        };

        // Create default trigger, component, and submit
        let trigger = TriggerJson::default();
        let component = ComponentJson::default();
        let submit = SubmitJson::default();

        // Create a new workflow entry
        let workflow = WorkflowJson {
            trigger,
            component,
            submit,
            aggregators: Vec::new(),
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
pub fn delete_workflow(file_path: &Path, workflow_id: WorkflowID) -> Result<WorkflowDeleteResult> {
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
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    event_type: String,
) -> Result<WorkflowTriggerResult> {
    // Parse the Cosmos address
    let address = query_client.chain_config.parse_address(&address_str)?;

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the trigger
        let trigger = Trigger::CosmosContractEvent {
            address,
            chain_name,
            event_type,
        };
        workflow.trigger = TriggerJson::Trigger(trigger.clone());

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
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    event_hash_str: String,
) -> Result<WorkflowTriggerResult> {
    // Parse the EVM address
    let address = alloy_primitives::Address::parse_checksummed(address_str, None)?;

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
            chain_name,
            event_hash: ByteArray::new(event_hash),
        };
        workflow.trigger = TriggerJson::Trigger(trigger.clone());

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
    workflow_id: WorkflowID,
    chain_name: ChainName,
    n_blocks: NonZeroU32,
) -> Result<WorkflowTriggerResult> {
    modify_service_file(file_path, |mut service| {
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        let trigger = Trigger::BlockInterval {
            chain_name,
            n_blocks,
        };
        workflow.trigger = TriggerJson::Trigger(trigger.clone());

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
    workflow_id: WorkflowID,
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
        workflow.trigger = TriggerJson::Trigger(trigger.clone());

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

/// Update component permissions
pub fn update_component_permissions(
    file_path: &Path,
    workflow_id: WorkflowID,
    http_hosts: Option<Vec<String>>,
    file_system: Option<bool>,
) -> Result<ComponentPermissionsResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the component exists
        let component = service
            .workflows
            .get_mut(&workflow_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
            })?
            .component
            .as_component_mut()
            .ok_or_else(|| {
                anyhow::anyhow!("Workflow with ID '{}' has unset component", workflow_id)
            })?;

        // Update HTTP permissions if specified
        if let Some(mut hosts) = http_hosts {
            // Sanitize inputs by trimming whitespace and removing empty strings
            hosts = hosts
                .into_iter()
                .map(|host| host.trim().to_string())
                .filter(|host| !host.is_empty())
                .collect();

            if hosts.is_empty() {
                // Empty list means no hosts allowed
                component.permissions.allowed_http_hosts = AllowedHostPermission::None;
            } else if hosts.len() == 1 && hosts[0] == "*" {
                // ["*"] means all hosts allowed
                component.permissions.allowed_http_hosts = AllowedHostPermission::All;
            } else {
                // List of specific hosts
                component.permissions.allowed_http_hosts = AllowedHostPermission::Only(hosts);
            }
        }

        // Update file system permission if specified
        if let Some(fs_perm) = file_system {
            component.permissions.file_system = fs_perm;
        }

        // Clone the updated permissions for the result
        let updated_permissions = component.permissions.clone();

        Ok((
            service,
            ComponentPermissionsResult {
                permissions: updated_permissions,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Update a component's fuel limit
pub fn update_component_fuel_limit(
    file_path: &Path,
    workflow_id: WorkflowID,
    fuel_limit: Option<u64>,
) -> Result<ComponentFuelLimitResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the component exists
        let component = service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?
            .component
            .as_component_mut()
            .context(format!(
                "Workflow with ID '{}' has unset component",
                workflow_id
            ))?;

        // Update the fuel limit
        component.fuel_limit = fuel_limit;

        Ok((
            service,
            ComponentFuelLimitResult {
                fuel_limit,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Update a component's configuration
pub fn update_component_config(
    file_path: &Path,
    workflow_id: WorkflowID,
    values: Option<Vec<String>>,
) -> Result<ComponentConfigResult> {
    modify_service_file(file_path, |mut service| {
        // First find the workflow and get a reference to it
        let workflow = service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?;

        // Now get a reference to the component
        let component = workflow.component.as_component_mut().context(format!(
            "Workflow with ID '{}' has unset component",
            workflow_id
        ))?;

        if let Some(values) = values {
            // If values provided, parse config values from 'key=value' format
            let mut config_pairs = BTreeMap::new();

            for value in values {
                match value.split_once('=') {
                    Some((key, value)) => {
                        // Trim whitespace and validate
                        let key = key.trim().to_string();
                        let value = value.trim().to_string();

                        if key.is_empty() {
                            return Err(anyhow::anyhow!("Empty key in config value: '{}'", value));
                        }

                        config_pairs.insert(key, value);
                    }
                    None => {
                        return Err(anyhow::anyhow!(
                            "Invalid config format: '{}'. Expected 'key=value'",
                            value
                        ));
                    }
                }
            }

            // Replace existing config with new values
            component.config = config_pairs;
        } else {
            // If no values provided, clear all config
            component.config.clear();
        }

        // Clone the updated config for the result
        let updated_config = component.config.clone();

        Ok((
            service,
            ComponentConfigResult {
                config: updated_config,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Update a component's maximum execution time
pub fn update_component_time_limit_seconds(
    file_path: &Path,
    workflow_id: WorkflowID,
    seconds: Option<u64>,
) -> Result<ComponentTimeLimitResult> {
    modify_service_file(file_path, |mut service| {
        // First find the workflow and get a reference to it
        let workflow = service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?;

        // Now get a reference to the component
        let component = workflow.component.as_component_mut().context(format!(
            "Workflow with ID '{}' has unset component",
            workflow_id
        ))?;

        // Update the maximum execution time
        component.time_limit_seconds = seconds;

        Ok((
            service,
            ComponentTimeLimitResult {
                time_limit_seconds: seconds,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Update a component's environment variable keys
pub fn update_component_env_keys(
    file_path: &Path,
    workflow_id: WorkflowID,
    values: Option<Vec<String>>,
) -> Result<ComponentEnvKeysResult> {
    modify_service_file(file_path, |mut service| {
        // First find the workflow and get a reference to it
        let workflow = service
            .workflows
            .get_mut(&workflow_id)
            .context(format!("No workflow id {workflow_id}"))?;

        // Now get a reference to the component
        let component = workflow.component.as_component_mut().context(format!(
            "Workflow with ID '{}' has unset component",
            workflow_id
        ))?;

        if let Some(values) = values {
            // Validate each environment variable to ensure it has the required prefix
            let mut validated_env_keys = BTreeSet::new();
            for key in values {
                let key = key.trim().to_string();

                if key.is_empty() {
                    continue; // Skip empty keys
                }

                if !key.starts_with(WAVS_ENV_PREFIX) {
                    return Err(anyhow::anyhow!(
                        "Environment variable '{}' must start with '{}'",
                        key,
                        WAVS_ENV_PREFIX
                    ));
                }

                validated_env_keys.insert(key);
            }

            // Replace existing env keys with new values
            component.env_keys = validated_env_keys;
        } else {
            // If no values provided, clear all env keys
            component.env_keys.clear();
        }

        // Clone the updated env keys for the result
        let updated_env_keys = component.env_keys.clone();

        Ok((
            service,
            ComponentEnvKeysResult {
                env_keys: updated_env_keys,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

pub fn set_evm_submit(
    file_path: &Path,
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    max_gas: Option<u64>,
) -> Result<WorkflowSubmitResult> {
    // Parse the EVM address
    let address = alloy_primitives::Address::parse_checksummed(address_str, None)?;

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the submit
        let submit = Submit::EvmContract(EvmContractSubmission {
            address,
            chain_name,
            max_gas,
        });
        workflow.submit = SubmitJson::Submit(submit.clone());

        // Reset the workflow aggregator
        workflow.aggregators = Vec::new();

        Ok((
            service,
            WorkflowSubmitResult {
                workflow_id,
                submit,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set an Aggregator submit for a workflow
pub fn set_aggregator_submit(
    file_path: &Path,
    workflow_id: WorkflowID,
    url: String,
    chain_name: ChainName,
    address_str: String,
    max_gas: Option<u64>,
) -> Result<WorkflowSubmitResult> {
    // Validate the URL format
    let _ = reqwest::Url::parse(&url).context(format!("Invalid URL format: {}", url))?;

    // Parse the EVM address
    let address = alloy_primitives::Address::parse_checksummed(address_str, None)?;

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the submit
        let submit = Submit::Aggregator { url };
        workflow.submit = SubmitJson::Submit(submit.clone());

        // Set the workflow aggregator
        workflow.aggregators = vec![Aggregator::Evm(EvmContractSubmission {
            chain_name,
            address,
            max_gas,
        })];

        Ok((
            service,
            WorkflowSubmitResult {
                workflow_id,
                submit,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Set an EVM manager for the service
pub fn set_evm_manager(
    file_path: &Path,
    address_str: String,
    chain_name: ChainName,
) -> Result<EvmManagerResult> {
    // Parse the EVM address
    let address = alloy_primitives::Address::parse_checksummed(address_str, None)?;

    modify_service_file(file_path, |mut service| {
        service.manager = ServiceManagerJson::Manager(ServiceManager::Evm {
            chain_name: chain_name.clone(),
            address,
        });

        Ok((
            service,
            EvmManagerResult {
                chain_name,
                address,
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
    let service: ServiceJson = serde_json::from_str(&service_json)?;

    // Get basic validation errors from the ServiceJson::validate method
    let mut errors = service.validate();

    // All remaining validation needs CliContext, so only do it if ctx is provided
    if let Some(ctx) = ctx {
        // Check component availability (can we download it?)
        validate_registry_availability(&ctx.config.wavs_endpoint, &mut errors).await;

        // Collect all triggers and submits for later validation
        let mut chains_to_validate = HashSet::new();
        let mut triggers = Vec::new();
        let mut submits = Vec::new();

        // Collect information for network-dependent validation
        for (workflow_id, workflow) in &service.workflows {
            if let TriggerJson::Trigger(trigger) = &workflow.trigger {
                match trigger {
                    Trigger::CosmosContractEvent { chain_name, .. } => {
                        chains_to_validate.insert((chain_name.clone(), ChainType::Cosmos));

                        // Cosmos-specific validation with client
                        if let Ok(client) = ctx.new_cosmos_client(chain_name).await {
                            validate_workflow_trigger(
                                workflow_id,
                                trigger,
                                &client.querier,
                                &mut errors,
                            )
                            .await;
                        } else {
                            errors.push(format!(
                                "Workflow '{}' uses chain '{}' in Cosmos trigger, but client configuration is invalid",
                                workflow_id, chain_name
                            ));
                        }
                    }
                    Trigger::EvmContractEvent { chain_name, .. } => {
                        chains_to_validate.insert((chain_name.clone(), ChainType::EVM));
                    }
                    _ => {}
                }

                // Collect trigger for contract existence check
                triggers.push((workflow_id, trigger));
            }

            if let SubmitJson::Submit(submit) = &workflow.submit {
                if let Submit::EvmContract(EvmContractSubmission { chain_name, .. }) = submit {
                    chains_to_validate.insert((chain_name.clone(), ChainType::EVM));
                }

                // Collect submit for contract existence check
                submits.push((workflow_id, submit));
            }

            for aggregator in &workflow.aggregators {
                match aggregator {
                    Aggregator::Evm(evm_contract_submission) => {
                        chains_to_validate
                            .insert((evm_contract_submission.chain_name.clone(), ChainType::EVM));
                    }
                };
            }
        }

        let service_manager = if let ServiceManagerJson::Manager(service_manager) = &service.manager
        {
            match service_manager {
                ServiceManager::Evm { chain_name, .. } => {
                    chains_to_validate.insert((chain_name.clone(), ChainType::EVM));
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
        for (chain_name, chain_type) in chains_to_validate.iter() {
            match chain_type {
                ChainType::Cosmos => {
                    if let Ok(client) = ctx.new_cosmos_client(chain_name).await {
                        cosmos_clients.insert(chain_name.clone(), client.querier);
                    }
                }
                ChainType::EVM => {
                    if let Ok(client) = ctx.new_evm_client(chain_name).await {
                        evm_providers.insert(chain_name.clone(), client.provider.root().clone());
                    }
                }
            }
        }

        // Validate that referenced contracts exist on-chain
        if !cosmos_clients.is_empty() || !evm_providers.is_empty() {
            if let Err(err) = validate_contracts_exist(
                service.id.as_ref(),
                triggers,
                submits,
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
        service_id: service.id.to_string(),
        errors,
    })
}

/// Validate a workflow trigger using a Cosmos query client
async fn validate_workflow_trigger(
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
async fn validate_registry_availability(registry_url: &str, errors: &mut Vec<String>) {
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
    service_id: &str,
    triggers: Vec<(&WorkflowID, &Trigger)>,
    submits: Vec<(&WorkflowID, &Submit)>,
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
                            format!("Service {} workflow {} trigger", service_id, workflow_id);
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
                            format!("Service {} workflow {} trigger", service_id, workflow_id);
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

    // Check all submit contracts
    for (workflow_id, submit) in submits {
        match submit {
            Submit::EvmContract(EvmContractSubmission {
                address,
                chain_name,
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
                            format!("Service {} workflow {} submit", service_id, workflow_id);
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
            Submit::None => {}
            Submit::Aggregator { url: _ } => {
                // TODO - anything to validate here?
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
                        let context = format!("Service {} manager", service_id);
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
async fn check_evm_contract_exists(
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
async fn check_cosmos_contract_exists(
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use crate::service_json::Json;

    use super::*;
    use alloy_primitives::hex;
    use layer_climb::prelude::{ChainConfig, ChainId};
    use layer_climb::querier::QueryClient as CosmosQueryClient;
    use tempfile::tempdir;

    #[test]
    fn test_service_init() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_service.json");

        // Initialize service
        let service_id = ServiceID::new("test-id-123").unwrap();
        let result = init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Verify the result
        assert_eq!(result.service.id, service_id);
        assert_eq!(result.service.name, "Test Service");
        assert_eq!(result.file_path, file_path);

        // Verify the file was created
        assert!(file_path.exists());

        // Parse the created file to verify its contents
        let file_content = std::fs::read_to_string(file_path).unwrap();
        let parsed_service: ServiceJson = serde_json::from_str(&file_content).unwrap();

        assert_eq!(parsed_service.id, service_id);
        assert_eq!(parsed_service.name, "Test Service");

        // Test with autogenerated ID
        let auto_id_file_path = temp_dir.path().join("auto_id_test.json");

        // Initialize service with no ID (should generate one)
        let auto_id_result =
            init_service(&auto_id_file_path, "Auto ID Service".to_string(), None).unwrap();

        // Verify service has generated ID
        assert!(!auto_id_result.service.id.is_empty());
        assert_eq!(auto_id_result.service.name, "Auto ID Service");

        // Verify file was created
        assert!(auto_id_file_path.exists());

        // Parse file to verify contents
        let auto_id_content = std::fs::read_to_string(auto_id_file_path).unwrap();
        let auto_id_parsed: ServiceJson = serde_json::from_str(&auto_id_content).unwrap();

        assert_eq!(auto_id_parsed.id, auto_id_result.service.id);
        assert_eq!(auto_id_parsed.name, "Auto ID Service");

        // Verify UUID format (should be UUID v7)
        let id_str = auto_id_parsed.id.to_string();
        assert!(id_str.len() > 30); // UUID should be reasonably long
        assert!(id_str.contains('-')); // Should have hyphens as per UUID format
    }

    #[test]
    fn test_workflow_component_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("component_operations_test.json");

        // Create a test digest - raw 64-character hex string (32 bytes)
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();

        // Initialize a service using the init_service method
        let service_id = ServiceID::new("test-service-id").unwrap();
        let init_result = init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Verify initialization result
        assert_eq!(init_result.service.id, service_id);
        assert_eq!(init_result.service.name, "Test Service");
        assert_eq!(init_result.file_path, file_path);

        // Need to have a valid workflow before we can work on its component
        let workflow_id = WorkflowID::new("workflow-1").unwrap();
        add_workflow(&file_path, Some(workflow_id.clone())).unwrap();

        // Test adding first component using Digest source
        let add_result =
            set_component_source_digest(&file_path, workflow_id.clone(), test_digest.clone())
                .unwrap();

        // Verify add result
        assert_eq!(add_result.digest, test_digest);
        assert_eq!(add_result.file_path, file_path);

        // Verify the file was modified by adding the component
        let service_after_add: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_add
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .is_set());

        // Test adding second component with Digest source
        let workflow_id_2 = WorkflowID::new("workflow-2").unwrap();
        add_workflow(&file_path, Some(workflow_id_2.clone())).unwrap();

        let second_add_result =
            set_component_source_digest(&file_path, workflow_id_2.clone(), test_digest.clone())
                .unwrap();

        // Verify second add result
        assert_eq!(second_add_result.digest, test_digest);

        // Test updating permissions - allow all HTTP hosts
        let permissions_result = update_component_permissions(
            &file_path,
            workflow_id.clone(),
            Some(vec!["*".to_string()]),
            Some(true),
        )
        .unwrap();

        // Verify permissions result
        assert!(permissions_result.permissions.file_system);
        assert!(matches!(
            permissions_result.permissions.allowed_http_hosts,
            AllowedHostPermission::All
        ));

        // Verify the service was updated with new permissions
        let service_after_permissions: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let updated_component = service_after_permissions
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert!(updated_component.permissions.file_system);
        assert!(matches!(
            updated_component.permissions.allowed_http_hosts,
            AllowedHostPermission::All
        ));

        // Test updating to specific HTTP hosts
        let specific_hosts = vec!["example.com".to_string(), "api.example.com".to_string()];
        let specific_hosts_result = update_component_permissions(
            &file_path,
            workflow_id_2.clone(),
            Some(specific_hosts.clone()),
            None,
        )
        .unwrap();

        // Verify specific hosts result
        if let AllowedHostPermission::Only(hosts) =
            &specific_hosts_result.permissions.allowed_http_hosts
        {
            assert_eq!(hosts.len(), 2);
            assert!(hosts.contains(&"example.com".to_string()));
            assert!(hosts.contains(&"api.example.com".to_string()));
        } else {
            panic!("Expected AllowedHostPermission::Only");
        }

        // Verify the service was updated with specific hosts
        let service_after_specific: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let specific_component = service_after_specific
            .workflows
            .get(&workflow_id_2)
            .unwrap()
            .component
            .as_component()
            .unwrap();

        if let AllowedHostPermission::Only(hosts) =
            &specific_component.permissions.allowed_http_hosts
        {
            assert_eq!(hosts.len(), 2);
            assert!(hosts.contains(&"example.com".to_string()));
            assert!(hosts.contains(&"api.example.com".to_string()));
        } else {
            panic!("Expected AllowedHostPermission::Only");
        }

        // Test updating to no HTTP hosts
        let no_hosts_result =
            update_component_permissions(&file_path, workflow_id.clone(), Some(vec![]), None)
                .unwrap();

        // Verify no hosts result
        assert!(matches!(
            no_hosts_result.permissions.allowed_http_hosts,
            AllowedHostPermission::None
        ));

        // Verify the service was updated with no hosts permission
        let service_after_no_hosts: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let no_hosts_component = service_after_no_hosts
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();

        assert!(matches!(
            no_hosts_component.permissions.allowed_http_hosts,
            AllowedHostPermission::None
        ));

        // Test error handling for permissions update with non-existent component
        let non_existent_id = WorkflowID::new("does-not-exist").unwrap();
        let error_permissions = update_component_permissions(
            &file_path,
            non_existent_id.clone(),
            Some(vec!["*".to_string()]),
            None,
        );

        // Verify error for permissions update
        assert!(error_permissions.is_err());
        let permissions_error = error_permissions.unwrap_err().to_string();
        assert!(permissions_error.contains(&non_existent_id.to_string()));
        assert!(permissions_error.contains("not found"));

        // Test adding third component with Digest source
        // Need to have a valid workflow before we can work on its component
        let workflow_id_3 = WorkflowID::new("workflow-3").unwrap();
        add_workflow(&file_path, Some(workflow_id_3.clone())).unwrap();

        let third_add_result =
            set_component_source_digest(&file_path, workflow_id_3.clone(), test_digest.clone())
                .unwrap();

        // Verify third add result
        assert_eq!(third_add_result.digest, test_digest);

        // Verify the third component was added
        let service_after_third_add: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_third_add
            .workflows
            .get(&workflow_id_3)
            .unwrap()
            .component
            .is_set());

        // Test setting a specific fuel limit
        let fuel_limit = 50000u64;
        let fuel_limit_result =
            update_component_fuel_limit(&file_path, workflow_id.clone(), Some(fuel_limit)).unwrap();

        // Verify fuel limit result
        assert_eq!(fuel_limit_result.fuel_limit, Some(fuel_limit));
        assert_eq!(fuel_limit_result.file_path, file_path);

        // Verify the service was updated with the fuel limit
        let service_after_fuel_limit: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_fuel_limit = service_after_fuel_limit
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(component_with_fuel_limit.fuel_limit, Some(fuel_limit));

        // Test removing a fuel limit (setting to None)
        let no_fuel_limit_result =
            update_component_fuel_limit(&file_path, workflow_id.clone(), None).unwrap();

        // Verify no fuel limit result
        assert_eq!(no_fuel_limit_result.fuel_limit, None);
        assert_eq!(no_fuel_limit_result.file_path, file_path);

        // Verify the service was updated with no fuel limit
        let service_after_no_fuel: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_no_fuel = service_after_no_fuel
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(component_with_no_fuel.fuel_limit, None);

        // Test adding configuration values
        let config_values = vec![
            "api_key=123456".to_string(),
            "timeout=30".to_string(),
            "debug=true".to_string(),
        ];
        let config_result =
            update_component_config(&file_path, workflow_id.clone(), Some(config_values.clone()))
                .unwrap();

        // Verify config result
        assert_eq!(config_result.config.len(), 3);
        assert!(config_result
            .config
            .iter()
            .any(|(k, v)| k == "api_key" && v == "123456"));
        assert!(config_result
            .config
            .iter()
            .any(|(k, v)| k == "timeout" && v == "30"));
        assert!(config_result
            .config
            .iter()
            .any(|(k, v)| k == "debug" && v == "true"));
        assert_eq!(config_result.file_path, file_path);

        // Verify the service was updated with the config
        let service_after_config: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_config = service_after_config
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(component_with_config.config.len(), 3);
        assert!(component_with_config
            .config
            .iter()
            .any(|(k, v)| k == "api_key" && v == "123456"));
        assert!(component_with_config
            .config
            .iter()
            .any(|(k, v)| k == "timeout" && v == "30"));
        assert!(component_with_config
            .config
            .iter()
            .any(|(k, v)| k == "debug" && v == "true"));

        // Test clearing configuration (set to None)
        let clear_config_result =
            update_component_config(&file_path, workflow_id.clone(), None).unwrap();

        // Verify clear config result
        assert_eq!(clear_config_result.config.len(), 0);
        assert_eq!(clear_config_result.file_path, file_path);

        // Verify the service was updated with empty config
        let service_after_clear_config: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_clear_config = service_after_clear_config
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(component_with_clear_config.config.len(), 0);

        // Test with invalid config format
        let invalid_config = vec!["invalid_format".to_string()];
        let invalid_config_result =
            update_component_config(&file_path, workflow_id.clone(), Some(invalid_config));

        // Verify it returns an error for invalid format
        assert!(invalid_config_result.is_err());
        let error_msg = invalid_config_result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid config format"));
        assert!(error_msg.contains("Expected 'key=value'"));

        // Test adding environment variables
        let env_keys = vec![
            "WAVS_ENV_API_KEY".to_string(),
            "WAVS_ENV_SECRET_TOKEN".to_string(),
            "WAVS_ENV_DATABASE_URL".to_string(),
        ];

        let env_result =
            update_component_env_keys(&file_path, workflow_id.clone(), Some(env_keys.clone()))
                .unwrap();

        // Verify env result
        assert_eq!(env_result.env_keys.len(), 3);
        assert!(env_result.env_keys.contains("WAVS_ENV_API_KEY"));
        assert!(env_result.env_keys.contains("WAVS_ENV_SECRET_TOKEN"));
        assert!(env_result.env_keys.contains("WAVS_ENV_DATABASE_URL"));

        // Verify the service was updated with env keys
        let service_after_env: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_env = service_after_env
            .workflows
            .get(&workflow_id)
            .unwrap()
            .component
            .as_component()
            .unwrap();

        assert_eq!(component_with_env.env_keys.len(), 3);
        assert!(component_with_env.env_keys.contains("WAVS_ENV_API_KEY"));
        assert!(component_with_env
            .env_keys
            .contains("WAVS_ENV_SECRET_TOKEN"));
        assert!(component_with_env
            .env_keys
            .contains("WAVS_ENV_DATABASE_URL"));

        // Test validation of env keys
        let invalid_env_keys = vec!["WAVS_ENV_VALID".to_string(), "INVALID_PREFIX".to_string()];

        let invalid_result =
            update_component_env_keys(&file_path, workflow_id.clone(), Some(invalid_env_keys));

        // Verify it returns an error for invalid prefix
        assert!(invalid_result.is_err());
        let error_msg = invalid_result.unwrap_err().to_string();
        assert!(error_msg.contains("must start with 'WAVS_ENV'"));

        // Test clearing env keys
        let clear_env_result =
            update_component_env_keys(&file_path, workflow_id.clone(), None).unwrap();

        // Verify clear result
        assert_eq!(clear_env_result.env_keys.len(), 0);

        // Test setting a specific max execution time
        let max_exec_time = 120u64;
        let max_exec_result = update_component_time_limit_seconds(
            &file_path,
            workflow_id_2.clone(),
            Some(max_exec_time),
        )
        .unwrap();

        // Verify max exec time result
        assert_eq!(max_exec_result.time_limit_seconds, Some(max_exec_time));
        assert_eq!(max_exec_result.file_path, file_path);

        // Verify the service was updated with max exec time
        let service_after_max_exec: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_max_exec = service_after_max_exec
            .workflows
            .get(&workflow_id_2)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(
            component_with_max_exec.time_limit_seconds,
            Some(max_exec_time)
        );

        // Test removing max exec time (setting to None)
        let no_max_exec_result =
            update_component_time_limit_seconds(&file_path, workflow_id_2.clone(), None).unwrap();

        // Verify no max exec time result
        assert_eq!(no_max_exec_result.time_limit_seconds, None);
        assert_eq!(no_max_exec_result.file_path, file_path);

        // Verify the service was updated with no max exec time
        let service_after_no_max_exec: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let component_with_no_max_exec = service_after_no_max_exec
            .workflows
            .get(&workflow_id_2)
            .unwrap()
            .component
            .as_component()
            .unwrap();
        assert_eq!(component_with_no_max_exec.time_limit_seconds, None);
    }

    #[test]
    fn test_workflow_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_operations_test.json");

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Test adding a workflow with specific ID
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        let add_result = add_workflow(&file_path, Some(workflow_id.clone())).unwrap();

        // Verify add result
        assert_eq!(add_result.workflow_id, workflow_id);
        assert_eq!(add_result.file_path, file_path);

        // Verify the file was modified by adding the workflow
        let service_after_add: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_add.workflows.contains_key(&workflow_id));
        assert_eq!(service_after_add.workflows.len(), 1);

        // Verify workflow properties - need to handle TriggerJson and SubmitJson wrappers
        let added_workflow = service_after_add.workflows.get(&workflow_id).unwrap();

        // Check trigger type with pattern matching for TriggerJson
        if let TriggerJson::Json(json) = &added_workflow.trigger {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Check component type
        assert!(added_workflow.component.is_unset());

        // Check submit type with pattern matching for SubmitJson
        if let SubmitJson::Json(json) = &added_workflow.submit {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Test adding a workflow with autogenerated ID
        let auto_id_result = add_workflow(&file_path, None).unwrap();
        let auto_workflow_id = auto_id_result.workflow_id;

        // Verify the auto-generated workflow was added
        let service_after_auto: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_auto.workflows.contains_key(&auto_workflow_id));
        assert_eq!(service_after_auto.workflows.len(), 2); // Two workflows now

        // Test deleting a workflow
        let delete_result = delete_workflow(&file_path, workflow_id.clone()).unwrap();

        // Verify delete result
        assert_eq!(delete_result.workflow_id, workflow_id);
        assert_eq!(delete_result.file_path, file_path);

        // Verify the file was modified by deleting the workflow
        let service_after_delete: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(!service_after_delete.workflows.contains_key(&workflow_id));
        assert_eq!(service_after_delete.workflows.len(), 1); // One workflow remaining

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let workflow_error = delete_workflow(&file_path, non_existent_workflow.clone());

        // Verify it returns an error with appropriate message
        assert!(workflow_error.is_err());
        let workflow_error_msg = workflow_error.unwrap_err().to_string();
        assert!(workflow_error_msg.contains(&non_existent_workflow.to_string()));
        assert!(workflow_error_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_workflow_trigger_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_trigger_test.json");

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a workflow
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        add_workflow(&file_path, Some(workflow_id.clone())).unwrap();

        // Initial workflow should have manual trigger (default when created)
        let service_initial: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let initial_workflow = service_initial.workflows.get(&workflow_id).unwrap();

        // Check the trigger type with proper handling of TriggerJson wrapper
        if let TriggerJson::Json(json) = &initial_workflow.trigger {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Create a mock CosmosQueryClient for testing
        let cosmos_chain_name = ChainName::from_str("cosmoshub-4").unwrap();
        let chain_config = ChainConfig {
            chain_id: ChainId::new(cosmos_chain_name.clone()),
            rpc_endpoint: Some("https://rpc.cosmos.network".to_string()),
            grpc_endpoint: Some("https://grpc.cosmos.network:443".to_string()),
            grpc_web_endpoint: Some("https://grpc-web.cosmos.network".to_string()),
            gas_price: 0.025,
            gas_denom: "uatom".to_string(),
            address_kind: layer_climb::prelude::AddrKind::Cosmos {
                prefix: "cosmos".to_string(),
            },
        };
        let query_client = CosmosQueryClient::new(chain_config, None)
            .await
            .expect("Failed to create Cosmos query client");

        // Test setting Cosmos trigger
        let cosmos_address = "cosmos1fl48vsnmsdzcv85q5d2q4z5ajdha8yu34mf0eh".to_string();
        let cosmos_event = "transfer".to_string();

        let cosmos_result = set_cosmos_trigger(
            query_client.clone(),
            &file_path,
            workflow_id.clone(),
            cosmos_address.clone(),
            cosmos_chain_name.clone(),
            cosmos_event.clone(),
        )
        .unwrap();

        // Verify cosmos trigger result
        assert_eq!(cosmos_result.workflow_id, workflow_id);
        if let Trigger::CosmosContractEvent {
            address,
            chain_name,
            event_type,
        } = &cosmos_result.trigger
        {
            assert_eq!(address.to_string(), cosmos_address);
            assert_eq!(chain_name, &cosmos_chain_name);
            assert_eq!(event_type, &cosmos_event);
        } else {
            panic!("Expected CosmosContractEvent trigger");
        }

        // Verify the service was updated with cosmos trigger
        let service_after_cosmos: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let cosmos_workflow = service_after_cosmos.workflows.get(&workflow_id).unwrap();

        // Handle TriggerJson wrapper
        if let TriggerJson::Trigger(trigger) = &cosmos_workflow.trigger {
            if let Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } = trigger
            {
                assert_eq!(address.to_string(), cosmos_address);
                assert_eq!(chain_name, &cosmos_chain_name);
                assert_eq!(event_type, &cosmos_event);
            } else {
                panic!("Expected CosmosContractEvent trigger in service");
            }
        } else {
            panic!("Expected TriggerJson::Trigger");
        }

        // Test for incorrect prefix - using Neutron (ntrn) prefix on Cosmos Hub
        let neutron_address = "ntrn1m8wnvy0jk8xf0hhn5uycrhjr3zpaqf4d0z9k8f".to_string();
        let wrong_prefix_result = set_cosmos_trigger(
            query_client.clone(),
            &file_path,
            workflow_id.clone(),
            neutron_address,
            cosmos_chain_name.clone(),
            cosmos_event.clone(),
        );

        // This should fail with a prefix validation error
        assert!(wrong_prefix_result.is_err());
        assert!(wrong_prefix_result
            .unwrap_err()
            .to_string()
            .contains("invalid bech32"),);

        // Test setting EVM trigger
        let evm_address = "0x00000000219ab540356cBB839Cbe05303d7705Fa".to_string();
        let evm_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let evm_event_hash =
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string();

        let evm_result = set_evm_trigger(
            &file_path,
            workflow_id.clone(),
            evm_address.clone(),
            evm_chain.clone(),
            evm_event_hash.clone(),
        )
        .unwrap();

        // Verify EVM trigger result
        assert_eq!(evm_result.workflow_id, workflow_id);
        if let Trigger::EvmContractEvent {
            address,
            chain_name,
            event_hash,
        } = &evm_result.trigger
        {
            assert_eq!(address.to_string(), evm_address);
            assert_eq!(chain_name, &evm_chain);
            // For event_hash we'll need to check the bytes match what we expect
            let expected_hash_bytes = hex::decode(evm_event_hash.trim_start_matches("0x")).unwrap();
            assert_eq!(event_hash.as_slice(), &expected_hash_bytes[..]);
        } else {
            panic!("Expected EvmContractEvent trigger");
        }

        // Verify the service was updated with EVM trigger
        let service_after: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let evm_workflow = service_after.workflows.get(&workflow_id).unwrap();

        // Handle TriggerJson wrapper
        if let TriggerJson::Trigger(trigger) = &evm_workflow.trigger {
            if let Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } = trigger
            {
                assert_eq!(address.to_string(), evm_address);
                assert_eq!(chain_name, &evm_chain);
                let expected_hash_bytes =
                    hex::decode(evm_event_hash.trim_start_matches("0x")).unwrap();
                assert_eq!(event_hash.as_slice(), &expected_hash_bytes[..]);
            } else {
                panic!("Expected EvmContractEvent trigger in service");
            }
        } else {
            panic!("Expected TriggerJson::Trigger");
        }

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let trigger_error = set_evm_trigger(
            &file_path,
            non_existent_workflow.clone(),
            evm_address.clone(),
            evm_chain.clone(),
            evm_event_hash.clone(),
        );

        // Verify it returns an error with appropriate message
        assert!(trigger_error.is_err());
        let trigger_error_msg = trigger_error.unwrap_err().to_string();
        assert!(trigger_error_msg.contains(&non_existent_workflow.to_string()));
        assert!(trigger_error_msg.contains("not found"));

        // Test error handling for invalid addresses
        let invalid_cosmos_address = "invalid-cosmos-address".to_string();
        let invalid_cosmos_result = set_cosmos_trigger(
            query_client, // Reuse the same query client
            &file_path,
            workflow_id.clone(),
            invalid_cosmos_address,
            cosmos_chain_name.clone(),
            cosmos_event.clone(),
        );
        assert!(invalid_cosmos_result.is_err());
        assert!(invalid_cosmos_result
            .unwrap_err()
            .to_string()
            .contains("invalid bech32"));

        let invalid_evm_address = "invalid-address".to_string();
        let invalid_evm_result = set_evm_trigger(
            &file_path,
            workflow_id.clone(),
            invalid_evm_address,
            evm_chain.clone(),
            evm_event_hash.clone(),
        );
        assert!(invalid_evm_result.is_err());
        assert!(invalid_evm_result
            .unwrap_err()
            .to_string()
            .contains("invalid string length"));
    }

    #[test]
    fn test_workflow_submit_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_submit_test.json");

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a workflow
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        add_workflow(&file_path, Some(workflow_id.clone())).unwrap();

        // Initial workflow should have None submit (default when created)
        let service_initial: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let initial_workflow = service_initial.workflows.get(&workflow_id).unwrap();

        // Handle SubmitJson wrapper
        if let SubmitJson::Json(json) = &initial_workflow.submit {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Test setting EVM submit
        let evm_address = "0x00000000219ab540356cBB839Cbe05303d7705Fa".to_string();
        let evm_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let max_gas = Some(1000000u64);

        let evm_result = set_evm_submit(
            &file_path,
            workflow_id.clone(),
            evm_address.clone(),
            evm_chain.clone(),
            max_gas,
        )
        .unwrap();

        // Verify EVM submit result
        assert_eq!(evm_result.workflow_id, workflow_id);
        if let Submit::EvmContract(EvmContractSubmission {
            address,
            chain_name,
            max_gas: result_max_gas,
        }) = &evm_result.submit
        {
            assert_eq!(address.to_string(), evm_address);
            assert_eq!(chain_name, &evm_chain);
            assert_eq!(result_max_gas, &max_gas);
        } else {
            panic!("Expected EvmServiceHandler submit");
        }

        // Verify the service was updated with EVM submit
        let service_after: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let evm_workflow = service_after.workflows.get(&workflow_id).unwrap();

        // Handle SubmitJson wrapper
        if let SubmitJson::Submit(submit) = &evm_workflow.submit {
            if let Submit::EvmContract(EvmContractSubmission {
                address,
                chain_name,
                max_gas: result_max_gas,
            }) = submit
            {
                assert_eq!(address.to_string(), evm_address);
                assert_eq!(chain_name, &evm_chain);
                assert_eq!(result_max_gas, &max_gas);
            } else {
                panic!("Expected EvmServiceHandler submit in service");
            }
        } else {
            panic!("Expected SubmitJson::Submit");
        }

        // Test updating with null max_gas
        let evm_result_no_gas = set_evm_submit(
            &file_path,
            workflow_id.clone(),
            evm_address.clone(),
            evm_chain.clone(),
            None,
        )
        .unwrap();

        // Verify EVM submit result without gas
        if let Submit::EvmContract(EvmContractSubmission {
            max_gas: result_max_gas,
            ..
        }) = &evm_result_no_gas.submit
        {
            assert_eq!(result_max_gas, &None);
        } else {
            panic!("Expected EvmServiceHandler submit");
        }

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let submit_error = set_evm_submit(
            &file_path,
            non_existent_workflow.clone(),
            evm_address.clone(),
            evm_chain.clone(),
            max_gas,
        );

        // Verify it returns an error with appropriate message
        assert!(submit_error.is_err());
        let submit_error_msg = submit_error.unwrap_err().to_string();
        assert!(submit_error_msg.contains(&non_existent_workflow.to_string()));
        assert!(submit_error_msg.contains("not found"));

        // Test error handling for invalid address
        let invalid_evm_address = "invalid-address".to_string();
        let invalid_evm_result = set_evm_submit(
            &file_path,
            workflow_id.clone(),
            invalid_evm_address,
            evm_chain.clone(),
            max_gas,
        );
        assert!(invalid_evm_result.is_err());
        let invalid_address_error = invalid_evm_result.unwrap_err().to_string();
        assert!(invalid_address_error.contains("invalid"));

        // Test setting Aggregator submit
        let aggregator_url = "https://api.example.com/aggregator".to_string();

        let aggregator_result = set_aggregator_submit(
            &file_path,
            workflow_id.clone(),
            aggregator_url.clone(),
            evm_chain.clone(),
            evm_address.clone(),
            None,
        )
        .unwrap();

        // Verify aggregator submit result
        assert_eq!(aggregator_result.workflow_id, workflow_id);
        if let Submit::Aggregator { url } = &aggregator_result.submit {
            assert_eq!(url, &aggregator_url);
        } else {
            panic!("Expected Aggregator submit");
        }

        // Verify the service was updated with aggregator submit
        let service_after_aggregator: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let aggregator_workflow = service_after_aggregator
            .workflows
            .get(&workflow_id)
            .unwrap();

        // Handle SubmitJson wrapper
        if let SubmitJson::Submit(submit) = &aggregator_workflow.submit {
            if let Submit::Aggregator { url } = submit {
                assert_eq!(url, &aggregator_url);
            } else {
                panic!("Expected Aggregator submit in service");
            }
        } else {
            panic!("Expected SubmitJson::Submit");
        }

        // Test error handling for invalid URL
        let invalid_url = "not-a-valid-url".to_string();
        let invalid_url_result = set_aggregator_submit(
            &file_path,
            workflow_id.clone(),
            invalid_url,
            evm_chain,
            evm_address,
            None,
        );
        assert!(invalid_url_result.is_err());
        let invalid_url_error = invalid_url_result.unwrap_err().to_string();
        assert!(invalid_url_error.contains("Invalid URL format"));
    }

    #[tokio::test]
    async fn test_service_validation() {
        // Create a temporary directory for test files
        let temp_dir = tempdir().unwrap();

        // Create common test objects
        let service_id = ServiceID::new("test-service-id").unwrap();
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        let evm_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let evm_address = alloy_primitives::Address::parse_checksummed(
            "0x00000000219ab540356cBB839Cbe05303d7705Fa",
            None,
        )
        .unwrap();

        // Create component with digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();
        let component = Component::new(ComponentSource::Digest(test_digest.clone()));

        // Create a valid trigger and submit for the workflow
        let trigger = Trigger::EvmContractEvent {
            address: evm_address,
            chain_name: evm_chain.clone(),
            event_hash: wavs_types::ByteArray::new([1u8; 32]),
        };

        let submit = Submit::EvmContract(EvmContractSubmission {
            address: evm_address,
            chain_name: evm_chain.clone(),
            max_gas: Some(1000000u64),
        });

        // Create service manager
        let manager = ServiceManagerJson::Manager(ServiceManager::Evm {
            chain_name: evm_chain.clone(),
            address: evm_address,
        });

        // Test valid service
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let valid_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("valid_service.json");
            let service_json = serde_json::to_string_pretty(&valid_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert_eq!(
                result.errors.len(),
                0,
                "Valid service should have no validation errors"
            );
        }

        // Test unset component
        {
            let mut workflows = BTreeMap::new();
            // Add original workflow
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            // Add invalid workflow with unset component
            let invalid_workflow_id = WorkflowID::new("invalid-workflow").unwrap();
            workflows.insert(
                invalid_workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::new_unset(),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let invalid_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("unset_component.json");
            let service_json = serde_json::to_string_pretty(&invalid_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Service with unset component should have errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains(&invalid_workflow_id.to_string())
                        && error.contains("has an unset component")),
                "Validation should catch missing component reference"
            );
        }

        // Test zero fuel limit
        {
            let mut workflows = BTreeMap::new();
            let mut zero_fuel_component = component.clone();
            zero_fuel_component.fuel_limit = Some(0);

            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(zero_fuel_component),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let zero_fuel_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("zero_fuel_service.json");
            let service_json = serde_json::to_string_pretty(&zero_fuel_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Zero fuel service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains(&workflow_id.to_string())
                        && error.contains("fuel limit of zero")),
                "Validation should catch zero fuel limit"
            );
        }

        // Test empty service name
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let empty_name_service = ServiceJson {
                id: service_id.clone(),
                name: "".to_string(), // Empty name
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("empty_name_service.json");
            let service_json = serde_json::to_string_pretty(&empty_name_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Empty name service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("Service name cannot be empty")),
                "Validation should catch empty service name"
            );
        }

        // Test invalid environment variable prefix
        {
            let mut workflows = BTreeMap::new();
            let mut env_component = component.clone();
            env_component.env_keys = ["INVALID_PREFIX_KEY".to_string()].into_iter().collect();

            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(env_component),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let invalid_env_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("invalid_env_prefix.json");
            let service_json = serde_json::to_string_pretty(&invalid_env_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Invalid env prefix service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("doesn't start with 'WAVS_ENV'")),
                "Validation should catch invalid environment variable prefix"
            );
        }

        // Test unset trigger
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Json(Json::Unset),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let unset_trigger_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("unset_trigger.json");
            let service_json = serde_json::to_string_pretty(&unset_trigger_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Unset trigger service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("has an unset trigger")),
                "Validation should catch unset trigger"
            );
        }

        // Test zero max_gas in submit
        {
            let mut workflows = BTreeMap::new();
            let zero_gas_submit = Submit::EvmContract(EvmContractSubmission {
                address: evm_address,
                chain_name: evm_chain.clone(),
                max_gas: Some(0u64), // Zero gas
            });

            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(zero_gas_submit),
                    aggregators: Vec::new(),
                },
            );

            let zero_gas_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("zero_max_gas.json");
            let service_json = serde_json::to_string_pretty(&zero_gas_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Zero max_gas service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("max_gas of zero")),
                "Validation should catch zero max_gas"
            );
        }

        // Test unset submit
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Json(Json::Unset),
                    aggregators: Vec::new(),
                },
            );

            let unset_submit_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("unset_submit.json");
            let service_json = serde_json::to_string_pretty(&unset_submit_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Unset submit service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("has an unset submit")),
                "Validation should catch unset submit"
            );
        }

        // Test invalid URL in Aggregator submit
        {
            let mut workflows = BTreeMap::new();
            let aggregators = vec![Aggregator::Evm(EvmContractSubmission {
                address: evm_address,
                chain_name: evm_chain.clone(),
                max_gas: Some(1000000u64),
            })];

            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(Submit::Aggregator {
                        url: "not-a-valid-url".to_string(),
                    }),
                    aggregators,
                },
            );

            let invalid_url_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("invalid_url.json");
            let service_json = serde_json::to_string_pretty(&invalid_url_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Invalid URL service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("invalid URL")),
                "Validation should catch invalid URL in Aggregator submit"
            );
        }

        // Test unset service manager
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(submit.clone()),
                    aggregators: Vec::new(),
                },
            );

            let unset_manager_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: ServiceManagerJson::Json(Json::Unset),
            };

            let file_path = temp_dir.path().join("unset_manager.json");
            let service_json = serde_json::to_string_pretty(&unset_manager_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "Unset manager service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("unset service manager")),
                "Validation should catch unset service manager"
            );
        }

        // Test submit with aggregator but no aggregator defined
        {
            let mut workflows = BTreeMap::new();
            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(Submit::Aggregator {
                        url: "https://example.com".to_string(),
                    }),
                    aggregators: Vec::new(), // No aggregator defined
                },
            );

            let no_aggregator_service = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("no_aggregator.json");
            let service_json = serde_json::to_string_pretty(&no_aggregator_service).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "No aggregator service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error
                        .contains("submits with aggregator, but no aggregator is defined")),
                "Validation should catch submit with aggregator but no aggregator defined"
            );
        }

        // Test no submit but aggregator defined
        {
            let mut workflows = BTreeMap::new();
            let aggregators = vec![Aggregator::Evm(EvmContractSubmission {
                address: evm_address,
                chain_name: evm_chain.clone(),
                max_gas: Some(1000000u64),
            })];

            workflows.insert(
                workflow_id.clone(),
                WorkflowJson {
                    trigger: TriggerJson::Trigger(trigger.clone()),
                    component: ComponentJson::Component(component.clone()),
                    submit: SubmitJson::Submit(Submit::None),
                    aggregators,
                },
            );

            let none_submit_with_aggregator = ServiceJson {
                id: service_id.clone(),
                name: "Test Service".to_string(),
                workflows,
                status: ServiceStatus::Active,
                manager: manager.clone(),
            };

            let file_path = temp_dir.path().join("none_submit_with_aggregator.json");
            let service_json = serde_json::to_string_pretty(&none_submit_with_aggregator).unwrap();
            std::fs::write(&file_path, service_json).unwrap();

            let result = validate_service(&file_path, None).await.unwrap();
            assert!(
                !result.errors.is_empty(),
                "None submit with aggregator service should have validation errors"
            );
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.contains("has no submit, but it has an aggregator defined")),
                "Validation should catch no submit but aggregator defined"
            );
        }
    }

    #[tokio::test]
    async fn test_set_component_source_registry() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("component_registry_test.json");

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a workflow
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        add_workflow(&file_path, Some(workflow_id.clone())).unwrap();

        // Test setting a component source to a registry
        let package = PackageRef::try_from("wavs-tests:square".to_string()).unwrap();

        // Use wa.dev as the registry domain (WAVS default)
        let registry_domain = "wa.dev".to_string();

        // Call the function to set the component source
        let result = set_component_source_registry(
            Some(registry_domain.clone()),
            &file_path,
            workflow_id.clone(),
            None, // Use default domain
            package.clone(),
            None, // Use latest version
        )
        .await;

        // Verify the operation was successful
        assert!(
            result.is_ok(),
            "Failed to set component source: {:?}",
            result.err()
        );

        let result = result.unwrap();

        // Basic validation of the result
        assert_eq!(result.package, package);
        assert_eq!(result.domain, registry_domain);
        assert!(
            !result.digest.to_string().is_empty(),
            "Digest should not be empty"
        );

        // Verify the service was updated with the registry source
        let service_json = std::fs::read_to_string(&file_path).unwrap();
        let service: ServiceJson = serde_json::from_str(&service_json).unwrap();

        let workflow = service.workflows.get(&workflow_id).unwrap();

        if let ComponentJson::Component(component) = &workflow.component {
            if let ComponentSource::Registry { registry } = &component.source {
                assert_eq!(registry.package, package);
                assert!(
                    !registry.digest.to_string().is_empty(),
                    "Digest in registry should not be empty"
                );
                assert_eq!(
                    registry.digest, result.digest,
                    "Digest in service file should match result digest"
                );
                // We used defaults here
                assert!(registry.version.is_none());
                assert!(registry.domain.is_none());
            } else {
                panic!("Expected Registry component source");
            }
        } else {
            panic!("Expected Component");
        }
    }
}
