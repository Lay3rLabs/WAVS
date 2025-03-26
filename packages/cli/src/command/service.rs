use alloy::providers::{Provider, RootProvider};
use alloy_json_abi::Event;
use anyhow::{Context as _, Result};
use layer_climb::{
    prelude::{Address, ConfigAddressExt as _},
    querier::QueryClient as CosmosQueryClient,
};
use reqwest::Client;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, ComponentID, ComponentSource, Digest,
    Permissions, ServiceConfig, ServiceID, ServiceStatus, Submit, Trigger, WorkflowID,
};

use crate::{
    args::{ComponentCommand, ServiceCommand, SubmitCommand, TriggerCommand, WorkflowCommand},
    context::CliContext,
    service_json::{ServiceJson, SubmitJson, TriggerJson, WorkflowJson},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChainType {
    Cosmos,
    Ethereum,
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
        ServiceCommand::Component { command } => match command {
            ComponentCommand::Add { id, digest } => {
                let result = add_component(&file, id, digest)?;
                display_result(ctx, result, &file, json)?;
            }
            ComponentCommand::Delete { id } => {
                let result = delete_component(&file, id)?;
                display_result(ctx, result, &file, json)?;
            }
            ComponentCommand::Permissions {
                id,
                http_hosts,
                file_system,
            } => {
                let result = update_component_permissions(&file, id, http_hosts, file_system)?;
                display_result(ctx, result, &file, json)?;
            }
        },
        ServiceCommand::Workflow { command } => match command {
            WorkflowCommand::Add {
                id,
                component_id,
                fuel_limit,
            } => {
                let result = add_workflow(&file, id, component_id, fuel_limit)?;
                display_result(ctx, result, &file, json)?;
            }
            WorkflowCommand::Delete { id } => {
                let result = delete_workflow(&file, id)?;
                display_result(ctx, result, &file, json)?;
            }
        },
        ServiceCommand::Trigger { command } => match command {
            TriggerCommand::SetCosmos {
                workflow_id,
                address,
                chain_name,
                event_type,
            } => {
                let query_client = ctx.get_cosmos_client(&chain_name)?.querier;
                let result = set_cosmos_trigger(
                    query_client,
                    &file,
                    workflow_id,
                    address,
                    chain_name,
                    event_type,
                )?;
                display_result(ctx, result, &file, json)?;
            }
            TriggerCommand::SetEthereum {
                workflow_id,
                address,
                chain_name,
                event_hash,
            } => {
                let result =
                    set_ethereum_trigger(&file, workflow_id, address, chain_name, event_hash)?;
                display_result(ctx, result, &file, json)?;
            }
        },
        ServiceCommand::Submit { command } => match command {
            SubmitCommand::SetEthereum {
                workflow_id,
                address,
                chain_name,
                max_gas,
            } => {
                let result = set_ethereum_submit(&file, workflow_id, address, chain_name, max_gas)?;
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

/// Result of adding a component
#[derive(Debug, Clone)]
pub struct ComponentAddResult {
    /// The component id
    pub component_id: ComponentID,
    /// The component digest
    pub digest: Digest,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentAddResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component added successfully!")?;
        writeln!(f, "  Component ID: {}", self.component_id)?;
        writeln!(f, "  Digest:       {}", self.digest)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
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

/// Result of deleting a component
#[derive(Debug, Clone)]
pub struct ComponentDeleteResult {
    /// The component id that was deleted
    pub component_id: ComponentID,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentDeleteResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component deleted successfully!")?;
        writeln!(f, "  Component ID: {}", self.component_id)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
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
            Trigger::EthContractEvent {
                address,
                chain_name,
                event_hash,
            } => {
                writeln!(f, "  Trigger Type: Ethereum Contract Event")?;
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
            Submit::EthereumContract {
                address,
                chain_name,
                max_gas,
            } => {
                writeln!(f, "  Submit Type: Ethereum Service Handler")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                if let Some(gas) = max_gas {
                    writeln!(f, "    Max Gas:    {}", gas)?;
                }
            }
            Submit::None => {
                writeln!(f, "  Submit Type: None")?;
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
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

/// Result of updating component permissions
#[derive(Debug, Clone)]
pub struct ComponentPermissionsResult {
    /// The component id that was edited
    pub component_id: ComponentID,
    /// The updated permissions
    pub permissions: Permissions,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentPermissionsResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component permissions updated successfully!")?;
        writeln!(f, "  Component ID: {}", self.component_id)?;

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
        components: BTreeMap::new(),
        workflows: BTreeMap::new(),
        status: ServiceStatus::Active,
        config: ServiceConfig::default(),
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

/// Add a component to a service
pub fn add_component(
    file_path: &Path,
    id: Option<ComponentID>,
    digest: Digest,
) -> Result<ComponentAddResult> {
    modify_service_file(file_path, |mut service| {
        // Generate component ID if not provided
        let component_id = match id {
            Some(id) => id,
            None => ComponentID::new(Uuid::now_v7().as_hyphenated().to_string())?,
        };

        // Create a new component entry
        let component = Component {
            source: ComponentSource::Digest(digest.clone()),
            permissions: Permissions::default(),
        };

        // Add the component to the service
        service.components.insert(component_id.clone(), component);

        Ok((
            service,
            ComponentAddResult {
                component_id,
                digest,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Delete a component from a service
pub fn delete_component(
    file_path: &Path,
    component_id: ComponentID,
) -> Result<ComponentDeleteResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the component exists
        if !service.components.contains_key(&component_id) {
            return Err(anyhow::anyhow!(
                "Component with ID '{}' not found in service",
                component_id
            ));
        }

        // Remove the component
        service.components.remove(&component_id);

        Ok((
            service,
            ComponentDeleteResult {
                component_id,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

/// Add a workflow to a service
pub fn add_workflow(
    file_path: &Path,
    id: Option<WorkflowID>,
    component_id: ComponentID,
    fuel_limit: Option<u64>,
) -> Result<WorkflowAddResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the component exists
        if !service.components.contains_key(&component_id) {
            return Err(anyhow::anyhow!(
                "Component with ID '{}' not found in service",
                component_id
            ));
        }

        // Generate workflow ID if not provided
        let workflow_id = match id {
            Some(id) => id,
            None => WorkflowID::new(Uuid::now_v7().as_hyphenated().to_string())?,
        };

        // Create default trigger and submit
        let trigger = TriggerJson::default();
        let submit = SubmitJson::default();

        // Create a new workflow entry
        let workflow = WorkflowJson {
            trigger,
            component: component_id,
            submit,
            fuel_limit,
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

/// Set an Ethereum contract event trigger for a workflow
pub fn set_ethereum_trigger(
    file_path: &Path,
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    event_hash_str: String,
) -> Result<WorkflowTriggerResult> {
    // Parse the Ethereum address
    let address = alloy::primitives::Address::parse_checksummed(address_str, None)?;

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
        let trigger = Trigger::EthContractEvent {
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

/// Update component permissions
pub fn update_component_permissions(
    file_path: &Path,
    component_id: ComponentID,
    http_hosts: Option<Vec<String>>,
    file_system: Option<bool>,
) -> Result<ComponentPermissionsResult> {
    modify_service_file(file_path, |mut service| {
        // Check if the component exists
        let component = service.components.get_mut(&component_id).ok_or_else(|| {
            anyhow::anyhow!("Component with ID '{}' not found in service", component_id)
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
                component_id,
                permissions: updated_permissions,
                file_path: file_path.to_path_buf(),
            },
        ))
    })
}

pub fn set_ethereum_submit(
    file_path: &Path,
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    max_gas: Option<u64>,
) -> Result<WorkflowSubmitResult> {
    // Parse the Ethereum address
    let address = alloy::primitives::Address::parse_checksummed(address_str, None)?;

    modify_service_file(file_path, |mut service| {
        // Check if the workflow exists
        let workflow = service.workflows.get_mut(&workflow_id).ok_or_else(|| {
            anyhow::anyhow!("Workflow with ID '{}' not found in service", workflow_id)
        })?;

        // Update the submit
        let submit = Submit::EthereumContract {
            address,
            chain_name,
            max_gas,
        };
        workflow.submit = SubmitJson::Submit(submit.clone());

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
                        if let Ok(client) = ctx.get_cosmos_client(chain_name) {
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
                    Trigger::EthContractEvent { chain_name, .. } => {
                        chains_to_validate.insert((chain_name.clone(), ChainType::Ethereum));
                    }
                    _ => {}
                }

                // Collect trigger for contract existence check
                triggers.push((workflow_id, trigger));
            }

            if let SubmitJson::Submit(submit) = &workflow.submit {
                if let Submit::EthereumContract { chain_name, .. } = submit {
                    chains_to_validate.insert((chain_name.clone(), ChainType::Ethereum));
                }

                // Collect submit for contract existence check
                submits.push((workflow_id, submit));
            }
        }

        // Build maps of clients for chains actually used
        let mut cosmos_clients = HashMap::new();
        let mut eth_providers = HashMap::new();

        // Only get clients for chains actually used in triggers or submits
        for (chain_name, chain_type) in chains_to_validate.iter() {
            match chain_type {
                ChainType::Cosmos => {
                    if let Ok(client) = ctx.get_cosmos_client(chain_name) {
                        cosmos_clients.insert(chain_name.clone(), client.querier);
                    }
                }
                ChainType::Ethereum => {
                    if let Ok(client) = ctx.get_eth_client(chain_name) {
                        eth_providers
                            .insert(chain_name.clone(), client.eth.provider.root().clone());
                    }
                }
            }
        }

        // Validate that referenced contracts exist on-chain
        if !cosmos_clients.is_empty() || !eth_providers.is_empty() {
            if let Err(err) = validate_contracts_exist(
                service.id.as_ref(),
                triggers,
                submits,
                &eth_providers,
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
    eth_providers: &HashMap<ChainName, RootProvider>,
    cosmos_clients: &HashMap<ChainName, CosmosQueryClient>,
    errors: &mut Vec<String>,
) -> Result<()> {
    // Track which contracts we've already checked to avoid duplicate checks
    let mut checked_eth_contracts = HashMap::new();
    let mut checked_cosmos_contracts = HashMap::new();

    // Check all trigger contracts
    for (workflow_id, trigger) in triggers {
        match trigger {
            Trigger::EthContractEvent {
                address,
                chain_name,
                ..
            } => {
                // Check if we have a provider for this chain
                if let Some(provider) = eth_providers.get(chain_name) {
                    // Only check each contract once per chain
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_eth_contracts.entry(key)
                    {
                        let context =
                            format!("Service {} workflow {} trigger", service_id, workflow_id);
                        match check_ethereum_contract_exists(address, provider, errors, &context)
                            .await
                        {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking Ethereum contract for workflow {}: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check Ethereum contract for workflow {} - no provider configured for chain {}",
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
            Trigger::Manual | Trigger::BlockInterval { .. } => {}
        }
    }

    // Check all submit contracts
    for (workflow_id, submit) in submits {
        match submit {
            Submit::EthereumContract {
                address,
                chain_name,
                ..
            } => {
                // Check if we have a provider for this chain
                if let Some(provider) = eth_providers.get(chain_name) {
                    // Only check each contract once per chain
                    let key = (address.to_string(), chain_name.to_string());
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        checked_eth_contracts.entry(key)
                    {
                        let context =
                            format!("Service {} workflow {} submit", service_id, workflow_id);
                        match check_ethereum_contract_exists(address, provider, errors, &context)
                            .await
                        {
                            Ok(exists) => {
                                e.insert(exists);
                            }
                            Err(err) => {
                                errors.push(format!(
                                    "Error checking Ethereum contract for workflow {} submit: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                    }
                } else {
                    errors.push(format!(
                        "Cannot check Ethereum contract for workflow {} submit - no provider configured for chain {}",
                        workflow_id, chain_name
                    ));
                }
            }
            Submit::None => {}
        }
    }

    Ok(())
}

/// Check if an Ethereum contract exists at the specified address
async fn check_ethereum_contract_exists(
    address: &alloy::primitives::Address,
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
                    "{}: Ethereum address {} has no contract deployed on chain (empty bytecode)",
                    context, address
                ));
            }
            Ok(exists)
        }
        Err(err) => {
            errors.push(format!(
                "{}: Failed to check Ethereum contract at {}: {} (RPC connection issue)",
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
    use alloy::hex;
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
    fn test_component_operations() {
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

        // Test adding first component using Digest source
        let component_id = ComponentID::new("component-123").unwrap();
        let add_result =
            add_component(&file_path, Some(component_id.clone()), test_digest.clone()).unwrap();

        // Verify add result
        assert_eq!(add_result.component_id, component_id);
        assert_eq!(add_result.digest, test_digest);
        assert_eq!(add_result.file_path, file_path);

        // Verify the file was modified by adding the component
        let service_after_add: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_add.components.contains_key(&component_id));
        assert_eq!(service_after_add.components.len(), 1);

        // Test adding second component with Digest source
        let second_component_id = ComponentID::new("component-456").unwrap();
        let second_add_result = add_component(
            &file_path,
            Some(second_component_id.clone()),
            test_digest.clone(),
        )
        .unwrap();

        // Verify second add result
        assert_eq!(second_add_result.component_id, second_component_id);
        assert_eq!(second_add_result.digest, test_digest);

        // Test updating permissions - allow all HTTP hosts
        let permissions_result = update_component_permissions(
            &file_path,
            component_id.clone(),
            Some(vec!["*".to_string()]),
            Some(true),
        )
        .unwrap();

        // Verify permissions result
        assert_eq!(permissions_result.component_id, component_id);
        assert!(permissions_result.permissions.file_system);
        assert!(matches!(
            permissions_result.permissions.allowed_http_hosts,
            AllowedHostPermission::All
        ));

        // Verify the service was updated with new permissions
        let service_after_permissions: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let updated_component = service_after_permissions
            .components
            .get(&component_id)
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
            second_component_id.clone(),
            Some(specific_hosts.clone()),
            None,
        )
        .unwrap();

        // Verify specific hosts result
        assert_eq!(specific_hosts_result.component_id, second_component_id);
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
            .components
            .get(&second_component_id)
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
            update_component_permissions(&file_path, component_id.clone(), Some(vec![]), None)
                .unwrap();

        // Verify no hosts result
        assert_eq!(no_hosts_result.component_id, component_id);
        assert!(matches!(
            no_hosts_result.permissions.allowed_http_hosts,
            AllowedHostPermission::None
        ));

        // Verify the service was updated with no hosts permission
        let service_after_no_hosts: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let no_hosts_component = service_after_no_hosts
            .components
            .get(&component_id)
            .unwrap();
        assert!(matches!(
            no_hosts_component.permissions.allowed_http_hosts,
            AllowedHostPermission::None
        ));

        // Test error handling for permissions update with non-existent component
        let non_existent_id = ComponentID::new("does-not-exist").unwrap();
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
        let third_component_id = ComponentID::new("component-789").unwrap();
        let third_add_result = add_component(
            &file_path,
            Some(third_component_id.clone()),
            test_digest.clone(),
        )
        .unwrap();

        // Verify third add result
        assert_eq!(third_add_result.component_id, third_component_id);
        assert_eq!(third_add_result.digest, test_digest);

        // Verify the third component was added
        let service_after_third_add: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_third_add
            .components
            .contains_key(&third_component_id));
        assert_eq!(service_after_third_add.components.len(), 3); // All three components

        // Test deleting a component
        let delete_result = delete_component(&file_path, component_id.clone()).unwrap();

        // Verify delete result
        assert_eq!(delete_result.component_id, component_id);
        assert_eq!(delete_result.file_path, file_path);

        // Verify the file was modified by deleting the component
        let service_after_delete: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(!service_after_delete.components.contains_key(&component_id));
        assert_eq!(service_after_delete.components.len(), 2); // One removed, two remaining

        // Test error handling for non-existent component
        let error_result = delete_component(&file_path, non_existent_id.clone());

        // Verify it returns an error with appropriate message
        assert!(error_result.is_err());
        let error_msg = error_result.unwrap_err().to_string();
        assert!(error_msg.contains(&non_existent_id.to_string()));
        assert!(error_msg.contains("not found"));
    }

    #[test]
    fn test_workflow_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_operations_test.json");

        // Create a test digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a component to use in workflows
        let component_id = ComponentID::new("component-123").unwrap();
        add_component(&file_path, Some(component_id.clone()), test_digest.clone()).unwrap();

        // Test adding a workflow with specific ID
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        let add_result = add_workflow(
            &file_path,
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

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
        assert_eq!(added_workflow.component, component_id);
        assert_eq!(added_workflow.fuel_limit, Some(1000));

        // Check trigger type with pattern matching for TriggerJson
        if let TriggerJson::Json(json) = &added_workflow.trigger {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Check submit type with pattern matching for SubmitJson
        if let SubmitJson::Json(json) = &added_workflow.submit {
            assert!(matches!(json, Json::Unset));
        } else {
            panic!("Expected Json::Unset");
        }

        // Test adding a workflow with autogenerated ID
        let auto_id_result = add_workflow(&file_path, None, component_id.clone(), None).unwrap();
        let auto_workflow_id = auto_id_result.workflow_id;

        // Verify the auto-generated workflow was added
        let service_after_auto: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_auto.workflows.contains_key(&auto_workflow_id));
        assert_eq!(service_after_auto.workflows.len(), 2); // Two workflows now

        // Test error when adding workflow with non-existent component
        let non_existent_component = ComponentID::new("does-not-exist").unwrap();
        let component_error = add_workflow(&file_path, None, non_existent_component.clone(), None);

        // Verify error for non-existent component
        assert!(component_error.is_err());
        let component_error_msg = component_error.unwrap_err().to_string();
        assert!(component_error_msg.contains(&non_existent_component.to_string()));
        assert!(component_error_msg.contains("not found"));

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

        // Create a test digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a component to use in workflows
        let component_id = ComponentID::new("component-123").unwrap();
        add_component(&file_path, Some(component_id.clone()), test_digest.clone()).unwrap();

        // Add a workflow
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        add_workflow(
            &file_path,
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

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

        // Test setting Ethereum trigger
        let eth_address = "0x00000000219ab540356cBB839Cbe05303d7705Fa".to_string();
        let eth_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let eth_event_hash =
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string();

        let eth_result = set_ethereum_trigger(
            &file_path,
            workflow_id.clone(),
            eth_address.clone(),
            eth_chain.clone(),
            eth_event_hash.clone(),
        )
        .unwrap();

        // Verify ethereum trigger result
        assert_eq!(eth_result.workflow_id, workflow_id);
        if let Trigger::EthContractEvent {
            address,
            chain_name,
            event_hash,
        } = &eth_result.trigger
        {
            assert_eq!(address.to_string(), eth_address);
            assert_eq!(chain_name, &eth_chain);
            // For event_hash we'll need to check the bytes match what we expect
            let expected_hash_bytes = hex::decode(eth_event_hash.trim_start_matches("0x")).unwrap();
            assert_eq!(event_hash.as_slice(), &expected_hash_bytes[..]);
        } else {
            panic!("Expected EthContractEvent trigger");
        }

        // Verify the service was updated with ethereum trigger
        let service_after_eth: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let eth_workflow = service_after_eth.workflows.get(&workflow_id).unwrap();

        // Handle TriggerJson wrapper
        if let TriggerJson::Trigger(trigger) = &eth_workflow.trigger {
            if let Trigger::EthContractEvent {
                address,
                chain_name,
                event_hash,
            } = trigger
            {
                assert_eq!(address.to_string(), eth_address);
                assert_eq!(chain_name, &eth_chain);
                let expected_hash_bytes =
                    hex::decode(eth_event_hash.trim_start_matches("0x")).unwrap();
                assert_eq!(event_hash.as_slice(), &expected_hash_bytes[..]);
            } else {
                panic!("Expected EthContractEvent trigger in service");
            }
        } else {
            panic!("Expected TriggerJson::Trigger");
        }

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let trigger_error = set_ethereum_trigger(
            &file_path,
            non_existent_workflow.clone(),
            eth_address.clone(),
            eth_chain.clone(),
            eth_event_hash.clone(),
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

        let invalid_eth_address = "invalid-eth-address".to_string();
        let invalid_eth_result = set_ethereum_trigger(
            &file_path,
            workflow_id.clone(),
            invalid_eth_address,
            eth_chain.clone(),
            eth_event_hash.clone(),
        );
        assert!(invalid_eth_result.is_err());
        assert!(invalid_eth_result
            .unwrap_err()
            .to_string()
            .contains("invalid string length"));
    }

    #[test]
    fn test_workflow_submit_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_submit_test.json");

        // Create a test digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            &file_path,
            "Test Service".to_string(),
            Some(service_id.clone()),
        )
        .unwrap();

        // Add a component to use in workflows
        let component_id = ComponentID::new("component-123").unwrap();
        add_component(&file_path, Some(component_id.clone()), test_digest.clone()).unwrap();

        // Add a workflow
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        add_workflow(
            &file_path,
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

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

        // Test setting Ethereum submit
        let eth_address = "0x00000000219ab540356cBB839Cbe05303d7705Fa".to_string();
        let eth_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let max_gas = Some(1000000u64);

        let eth_result = set_ethereum_submit(
            &file_path,
            workflow_id.clone(),
            eth_address.clone(),
            eth_chain.clone(),
            max_gas,
        )
        .unwrap();

        // Verify ethereum submit result
        assert_eq!(eth_result.workflow_id, workflow_id);
        if let Submit::EthereumContract {
            address,
            chain_name,
            max_gas: result_max_gas,
        } = &eth_result.submit
        {
            assert_eq!(address.to_string(), eth_address);
            assert_eq!(chain_name, &eth_chain);
            assert_eq!(result_max_gas, &max_gas);
        } else {
            panic!("Expected EthServiceHandler submit");
        }

        // Verify the service was updated with ethereum submit
        let service_after_eth: ServiceJson =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let eth_workflow = service_after_eth.workflows.get(&workflow_id).unwrap();

        // Handle SubmitJson wrapper
        if let SubmitJson::Submit(submit) = &eth_workflow.submit {
            if let Submit::EthereumContract {
                address,
                chain_name,
                max_gas: result_max_gas,
            } = submit
            {
                assert_eq!(address.to_string(), eth_address);
                assert_eq!(chain_name, &eth_chain);
                assert_eq!(result_max_gas, &max_gas);
            } else {
                panic!("Expected EthServiceHandler submit in service");
            }
        } else {
            panic!("Expected SubmitJson::Submit");
        }

        // Test updating with null max_gas
        let eth_result_no_gas = set_ethereum_submit(
            &file_path,
            workflow_id.clone(),
            eth_address.clone(),
            eth_chain.clone(),
            None,
        )
        .unwrap();

        // Verify ethereum submit result without gas
        if let Submit::EthereumContract {
            max_gas: result_max_gas,
            ..
        } = &eth_result_no_gas.submit
        {
            assert_eq!(result_max_gas, &None);
        } else {
            panic!("Expected EthServiceHandler submit");
        }

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let submit_error = set_ethereum_submit(
            &file_path,
            non_existent_workflow.clone(),
            eth_address.clone(),
            eth_chain.clone(),
            max_gas,
        );

        // Verify it returns an error with appropriate message
        assert!(submit_error.is_err());
        let submit_error_msg = submit_error.unwrap_err().to_string();
        assert!(submit_error_msg.contains(&non_existent_workflow.to_string()));
        assert!(submit_error_msg.contains("not found"));

        // Test error handling for invalid address
        let invalid_eth_address = "invalid-eth-address".to_string();
        let invalid_eth_result = set_ethereum_submit(
            &file_path,
            workflow_id.clone(),
            invalid_eth_address,
            eth_chain.clone(),
            max_gas,
        );
        assert!(invalid_eth_result.is_err());
        let invalid_address_error = invalid_eth_result.unwrap_err().to_string();
        assert!(invalid_address_error.contains("invalid"));
    }

    #[tokio::test]
    async fn test_service_validation() {
        // Create a temporary directory for test files
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_service.json");

        // Create a valid service configuration
        let service_id = ServiceID::new("test-service-id").unwrap();
        let component_id = ComponentID::new("component-123").unwrap();
        let workflow_id = WorkflowID::new("workflow-123").unwrap();
        let ethereum_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let ethereum_address = alloy::primitives::Address::parse_checksummed(
            "0x00000000219ab540356cBB839Cbe05303d7705Fa",
            None,
        )
        .unwrap();

        // Create component with digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();
        let component = Component {
            source: ComponentSource::Digest(test_digest.clone()),
            permissions: Permissions::default(),
        };

        // Create a valid trigger for the workflow
        let trigger = Trigger::EthContractEvent {
            address: ethereum_address,
            chain_name: ethereum_chain.clone(),
            event_hash: wavs_types::ByteArray::new([1u8; 32]),
        };

        // Create a valid submit for the workflow
        let submit = Submit::EthereumContract {
            address: ethereum_address,
            chain_name: ethereum_chain.clone(),
            max_gas: Some(1000000u64),
        };

        // Create workflow with the trigger and submit
        let workflow = WorkflowJson {
            trigger: TriggerJson::Trigger(trigger.clone()),
            component: component_id.clone(),
            submit: SubmitJson::Submit(submit.clone()),
            fuel_limit: Some(1000),
        };

        // Create a valid service
        let mut components = BTreeMap::new();
        components.insert(component_id.clone(), component);

        let mut workflows = BTreeMap::new();
        workflows.insert(workflow_id.clone(), workflow);

        let service = ServiceJson {
            id: service_id.clone(),
            name: "Test Service".to_string(),
            components,
            workflows,
            status: ServiceStatus::Active,
            config: ServiceConfig::default(),
        };

        // Write the service to a file
        let service_json = serde_json::to_string_pretty(&service).unwrap();
        let mut file = File::create(&file_path).unwrap();
        file.write_all(service_json.as_bytes()).unwrap();

        // Validate the service - this should pass with no errors since we've created a valid service
        // Note: Using None for ctx since we can't easily mock connection to blockchain
        let result = validate_service(&file_path, None).await.unwrap();

        // Check that validation succeeds
        assert_eq!(
            result.errors.len(),
            0,
            "Valid service should have no validation errors"
        );
        assert_eq!(result.service_id, service_id.to_string());

        // Create an invalid service with missing component reference
        let mut invalid_service = service.clone();

        // Create a new workflow that references a non-existent component
        let non_existent_component_id = ComponentID::new("does-not-exist").unwrap();
        let invalid_workflow = WorkflowJson {
            trigger: TriggerJson::Trigger(trigger.clone()),
            component: non_existent_component_id.clone(),
            submit: SubmitJson::Submit(submit.clone()),
            fuel_limit: Some(1000),
        };

        let invalid_workflow_id = WorkflowID::new("invalid-workflow").unwrap();
        invalid_service
            .workflows
            .insert(invalid_workflow_id.clone(), invalid_workflow);

        // Write the invalid service to a file
        let invalid_service_path = temp_dir.path().join("invalid_service.json");
        let invalid_service_json = serde_json::to_string_pretty(&invalid_service).unwrap();
        let mut invalid_file = File::create(&invalid_service_path).unwrap();
        invalid_file
            .write_all(invalid_service_json.as_bytes())
            .unwrap();

        // Validate the service - this should fail with component reference error
        let invalid_result = validate_service(&invalid_service_path, None).await.unwrap();

        // Check that validation fails with appropriate error
        assert!(
            !invalid_result.errors.is_empty(),
            "Invalid service should have validation errors"
        );
        let component_error = invalid_result.errors.iter().any(|error| {
            error.contains(&invalid_workflow_id.to_string())
                && error.contains(&non_existent_component_id.to_string())
                && error.contains("non-existent component")
        });
        assert!(
            component_error,
            "Validation should catch missing component reference"
        );

        // Create an invalid service with zero fuel limit
        let mut zero_fuel_service = service.clone();

        // Modify the workflow to have a zero fuel limit
        let zero_fuel_workflow = WorkflowJson {
            trigger: TriggerJson::Trigger(trigger),
            component: component_id.clone(),
            submit: SubmitJson::Submit(submit),
            fuel_limit: Some(0), // Invalid - zero fuel limit
        };

        zero_fuel_service.workflows.clear();
        zero_fuel_service
            .workflows
            .insert(workflow_id.clone(), zero_fuel_workflow);

        // Write the zero fuel service to a file
        let zero_fuel_path = temp_dir.path().join("zero_fuel_service.json");
        let zero_fuel_json = serde_json::to_string_pretty(&zero_fuel_service).unwrap();
        let mut zero_fuel_file = File::create(&zero_fuel_path).unwrap();
        zero_fuel_file.write_all(zero_fuel_json.as_bytes()).unwrap();

        // Validate the service - this should fail with fuel limit error
        let zero_fuel_result = validate_service(&zero_fuel_path, None).await.unwrap();

        // Check that validation fails with appropriate error
        assert!(
            !zero_fuel_result.errors.is_empty(),
            "Zero fuel service should have validation errors"
        );
        let fuel_error = zero_fuel_result.errors.iter().any(|error| {
            error.contains(&workflow_id.to_string()) && error.contains("fuel limit of zero")
        });
        assert!(fuel_error, "Validation should catch zero fuel limit");

        // Create a service with empty name
        let mut empty_name_service = service.clone();
        empty_name_service.name = "".to_string(); // Invalid - empty name

        // Write the empty name service to a file
        let empty_name_path = temp_dir.path().join("empty_name_service.json");
        let empty_name_json = serde_json::to_string_pretty(&empty_name_service).unwrap();
        let mut empty_name_file = File::create(&empty_name_path).unwrap();
        empty_name_file
            .write_all(empty_name_json.as_bytes())
            .unwrap();

        // Validate the service - this should fail with name error
        let empty_name_result = validate_service(&empty_name_path, None).await.unwrap();

        // Check that validation fails with appropriate error
        assert!(
            !empty_name_result.errors.is_empty(),
            "Empty name service should have validation errors"
        );
        let name_error = empty_name_result
            .errors
            .iter()
            .any(|error| error.contains("Service name cannot be empty"));
        assert!(name_error, "Validation should catch empty service name");
    }
}
