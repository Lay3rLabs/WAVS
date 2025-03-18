use alloy_json_abi::Event;
use anyhow::{Context as _, Result};
use layer_climb::{prelude::ConfigAddressExt as _, querier::QueryClient as CosmosQueryClient};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, ComponentID, ComponentSource, Digest,
    Permissions, Service, ServiceConfig, ServiceID, ServiceStatus, Submit, Trigger, Workflow,
    WorkflowID,
};

use crate::{
    args::{ComponentCommand, ServiceCommand, SubmitCommand, TriggerCommand, WorkflowCommand},
    context::CliContext,
};

/// Handle service commands - this function will be called from main.rs
pub async fn handle_service_command(
    ctx: &CliContext,
    file: PathBuf,
    command: ServiceCommand,
) -> Result<()> {
    match command {
        ServiceCommand::Init { name, id } => {
            let result = init_service(file, name, id)?;
            ctx.handle_display_result(result);
        }
        ServiceCommand::Component { command } => match command {
            ComponentCommand::Add { id, digest } => {
                let result = add_component(&file, id, digest)?;
                ctx.handle_display_result(result);
            }
            ComponentCommand::Delete { id } => {
                let result = delete_component(&file, id)?;
                ctx.handle_display_result(result);
            }
            ComponentCommand::Permissions {
                id,
                http_hosts,
                file_system,
            } => {
                let result = update_component_permissions(file, id, http_hosts, file_system)?;
                ctx.handle_display_result(result);
            }
        },
        ServiceCommand::Workflow { command } => match command {
            WorkflowCommand::Add {
                id,
                component_id,
                fuel_limit,
            } => {
                let result = add_workflow(file, id, component_id, fuel_limit)?;
                ctx.handle_display_result(result);
            }
            WorkflowCommand::Delete { id } => {
                let result = delete_workflow(file, id)?;
                ctx.handle_display_result(result);
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
                    file,
                    workflow_id,
                    address,
                    chain_name,
                    event_type,
                )?;
                ctx.handle_display_result(result);
            }
            TriggerCommand::SetEthereum {
                workflow_id,
                address,
                chain_name,
                event_hash,
            } => {
                let result =
                    set_ethereum_trigger(file, workflow_id, address, chain_name, event_hash)?;
                ctx.handle_display_result(result);
            }
        },
        ServiceCommand::Submit { command } => match command {
            SubmitCommand::SetEthereum {
                workflow_id,
                address,
                chain_name,
                max_gas,
            } => {
                let result = set_ethereum_submit(file, workflow_id, address, chain_name, max_gas)?;
                ctx.handle_display_result(result);
            }
        },
    }

    Ok(())
}

/// Result of service initialization
#[derive(Debug, Clone)]
pub struct ServiceInitResult {
    /// The generated service
    pub service: Service,
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

/// Helper function to load a service, modify it, and save it back
pub fn modify_service_file<P, F, R>(file_path: P, modifier: F) -> Result<R>
where
    P: AsRef<Path>,
    F: FnOnce(Service) -> Result<(Service, R)>,
{
    let file_path = file_path.as_ref();

    // Read the service file
    let service_json = std::fs::read_to_string(file_path)?;

    // Parse the service JSON
    let service: Service = serde_json::from_str(&service_json)?;

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
    file_path: PathBuf,
    name: String,
    id: Option<ServiceID>,
) -> Result<ServiceInitResult> {
    // Generate service ID if not provided
    let id = match id {
        Some(id) => id,
        None => ServiceID::new(Uuid::now_v7().as_hyphenated().to_string())?,
    };

    // Create the service
    let service = Service {
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
    let mut file = File::create(&file_path)?;
    file.write_all(service_json.as_bytes())?;

    Ok(ServiceInitResult { service, file_path })
}

/// Add a component to a service
pub fn add_component(
    file_path: &PathBuf,
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
                file_path: file_path.clone(),
            },
        ))
    })
}

/// Delete a component from a service
pub fn delete_component(
    file_path: &PathBuf,
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
                file_path: file_path.clone(),
            },
        ))
    })
}

/// Add a workflow to a service
pub fn add_workflow(
    file_path: PathBuf,
    id: Option<WorkflowID>,
    component_id: ComponentID,
    fuel_limit: Option<u64>,
) -> Result<WorkflowAddResult> {
    modify_service_file(file_path.clone(), |mut service| {
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
        let trigger = Trigger::Manual;
        let submit = Submit::None;

        // Create a new workflow entry
        let workflow = Workflow {
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
                file_path,
            },
        ))
    })
}

/// Delete a workflow from a service
pub fn delete_workflow(
    file_path: PathBuf,
    workflow_id: WorkflowID,
) -> Result<WorkflowDeleteResult> {
    modify_service_file(file_path.clone(), |mut service| {
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
                file_path,
            },
        ))
    })
}

/// Set a Cosmos contract event trigger for a workflow
pub fn set_cosmos_trigger(
    query_client: CosmosQueryClient,
    file_path: PathBuf,
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    event_type: String,
) -> Result<WorkflowTriggerResult> {
    // Parse the Cosmos address
    let address = query_client.chain_config.parse_address(&address_str)?;

    modify_service_file(file_path.clone(), |mut service| {
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
        workflow.trigger = trigger.clone();

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path,
            },
        ))
    })
}

/// Set an Ethereum contract event trigger for a workflow
pub fn set_ethereum_trigger(
    file_path: PathBuf,
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

    modify_service_file(file_path.clone(), |mut service| {
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
        workflow.trigger = trigger.clone();

        Ok((
            service,
            WorkflowTriggerResult {
                workflow_id,
                trigger,
                file_path,
            },
        ))
    })
}

/// Update component permissions
pub fn update_component_permissions(
    file_path: PathBuf,
    component_id: ComponentID,
    http_hosts: Option<Vec<String>>,
    file_system: Option<bool>,
) -> Result<ComponentPermissionsResult> {
    modify_service_file(file_path.clone(), |mut service| {
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
                file_path,
            },
        ))
    })
}

pub fn set_ethereum_submit(
    file_path: PathBuf,
    workflow_id: WorkflowID,
    address_str: String,
    chain_name: ChainName,
    max_gas: Option<u64>,
) -> Result<WorkflowSubmitResult> {
    // Parse the Ethereum address
    let address = alloy::primitives::Address::parse_checksummed(address_str, None)?;

    modify_service_file(file_path.clone(), |mut service| {
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
        workflow.submit = submit.clone();

        Ok((
            service,
            WorkflowSubmitResult {
                workflow_id,
                submit,
                file_path,
            },
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

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
            file_path.clone(),
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
        let parsed_service: Service = serde_json::from_str(&file_content).unwrap();

        assert_eq!(parsed_service.id, service_id);
        assert_eq!(parsed_service.name, "Test Service");

        // Test with autogenerated ID
        let auto_id_file_path = temp_dir.path().join("auto_id_test.json");

        // Initialize service with no ID (should generate one)
        let auto_id_result = init_service(
            auto_id_file_path.clone(),
            "Auto ID Service".to_string(),
            None,
        )
        .unwrap();

        // Verify service has generated ID
        assert!(!auto_id_result.service.id.is_empty());
        assert_eq!(auto_id_result.service.name, "Auto ID Service");

        // Verify file was created
        assert!(auto_id_file_path.exists());

        // Parse file to verify contents
        let auto_id_content = std::fs::read_to_string(auto_id_file_path).unwrap();
        let auto_id_parsed: Service = serde_json::from_str(&auto_id_content).unwrap();

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
            file_path.clone(),
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
        let service_after_add: Service =
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
            file_path.clone(),
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
        let service_after_permissions: Service =
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
            file_path.clone(),
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
        let service_after_specific: Service =
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
        let no_hosts_result = update_component_permissions(
            file_path.clone(),
            component_id.clone(),
            Some(vec![]),
            None,
        )
        .unwrap();

        // Verify no hosts result
        assert_eq!(no_hosts_result.component_id, component_id);
        assert!(matches!(
            no_hosts_result.permissions.allowed_http_hosts,
            AllowedHostPermission::None
        ));

        // Verify the service was updated with no hosts permission
        let service_after_no_hosts: Service =
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
            file_path.clone(),
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
        let service_after_third_add: Service =
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
        let service_after_delete: Service =
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
            file_path.clone(),
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
            file_path.clone(),
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

        // Verify add result
        assert_eq!(add_result.workflow_id, workflow_id);
        assert_eq!(add_result.file_path, file_path);

        // Verify the file was modified by adding the workflow
        let service_after_add: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_add.workflows.contains_key(&workflow_id));
        assert_eq!(service_after_add.workflows.len(), 1);

        // Verify workflow properties
        let added_workflow = service_after_add.workflows.get(&workflow_id).unwrap();
        assert_eq!(added_workflow.component, component_id);
        assert_eq!(added_workflow.fuel_limit, Some(1000));
        assert!(matches!(added_workflow.trigger, Trigger::Manual));
        assert!(matches!(added_workflow.submit, Submit::None));

        // Test adding a workflow with autogenerated ID
        let auto_id_result =
            add_workflow(file_path.clone(), None, component_id.clone(), None).unwrap();
        let auto_workflow_id = auto_id_result.workflow_id;

        // Verify the auto-generated workflow was added
        let service_after_auto: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_auto.workflows.contains_key(&auto_workflow_id));
        assert_eq!(service_after_auto.workflows.len(), 2); // Two workflows now

        // Test error when adding workflow with non-existent component
        let non_existent_component = ComponentID::new("does-not-exist").unwrap();
        let component_error = add_workflow(
            file_path.clone(),
            None,
            non_existent_component.clone(),
            None,
        );

        // Verify error for non-existent component
        assert!(component_error.is_err());
        let component_error_msg = component_error.unwrap_err().to_string();
        assert!(component_error_msg.contains(&non_existent_component.to_string()));
        assert!(component_error_msg.contains("not found"));

        // Test deleting a workflow
        let delete_result = delete_workflow(file_path.clone(), workflow_id.clone()).unwrap();

        // Verify delete result
        assert_eq!(delete_result.workflow_id, workflow_id);
        assert_eq!(delete_result.file_path, file_path);

        // Verify the file was modified by deleting the workflow
        let service_after_delete: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(!service_after_delete.workflows.contains_key(&workflow_id));
        assert_eq!(service_after_delete.workflows.len(), 1); // One workflow remaining

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let workflow_error = delete_workflow(file_path.clone(), non_existent_workflow.clone());

        // Verify it returns an error with appropriate message
        assert!(workflow_error.is_err());
        let workflow_error_msg = workflow_error.unwrap_err().to_string();
        assert!(workflow_error_msg.contains(&non_existent_workflow.to_string()));
        assert!(workflow_error_msg.contains("not found"));
    }

    #[test]
    fn test_workflow_trigger_operations() {
        // Create a temporary directory and file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("workflow_trigger_test.json");

        // Create a runtime for async operations
        let rt = tokio::runtime::Runtime::new().unwrap();

        // Create a test digest
        let test_digest =
            Digest::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
                .unwrap();

        // Initialize a service
        let service_id = ServiceID::new("test-service-id").unwrap();
        init_service(
            file_path.clone(),
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
            file_path.clone(),
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

        // Initial workflow should have manual trigger (default when created)
        let service_initial: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let initial_workflow = service_initial.workflows.get(&workflow_id).unwrap();
        assert!(matches!(initial_workflow.trigger, Trigger::Manual));

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
        let query_client = rt.block_on(async {
            CosmosQueryClient::new(chain_config, None)
                .await
                .expect("Failed to create Cosmos query client")
        });

        // Test setting Cosmos trigger
        let cosmos_address = "cosmos1fl48vsnmsdzcv85q5d2q4z5ajdha8yu34mf0eh".to_string();
        let cosmos_event = "transfer".to_string();

        let cosmos_result = set_cosmos_trigger(
            query_client.clone(),
            file_path.clone(),
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
        let service_after_cosmos: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let cosmos_workflow = service_after_cosmos.workflows.get(&workflow_id).unwrap();
        if let Trigger::CosmosContractEvent {
            address,
            chain_name,
            event_type,
        } = &cosmos_workflow.trigger
        {
            assert_eq!(address.to_string(), cosmos_address);
            assert_eq!(chain_name, &cosmos_chain_name);
            assert_eq!(event_type, &cosmos_event);
        } else {
            panic!("Expected CosmosContractEvent trigger in service");
        }

        // Test for incorrect prefix - using Neutron (ntrn) prefix on Cosmos Hub
        let neutron_address = "ntrn1m8wnvy0jk8xf0hhn5uycrhjr3zpaqf4d0z9k8f".to_string();
        let wrong_prefix_result = set_cosmos_trigger(
            query_client.clone(),
            file_path.clone(),
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
            file_path.clone(),
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
        let service_after_eth: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let eth_workflow = service_after_eth.workflows.get(&workflow_id).unwrap();
        if let Trigger::EthContractEvent {
            address,
            chain_name,
            event_hash,
        } = &eth_workflow.trigger
        {
            assert_eq!(address.to_string(), eth_address);
            assert_eq!(chain_name, &eth_chain);
            let expected_hash_bytes = hex::decode(eth_event_hash.trim_start_matches("0x")).unwrap();
            assert_eq!(event_hash.as_slice(), &expected_hash_bytes[..]);
        } else {
            panic!("Expected EthContractEvent trigger in service");
        }

        // Test error handling for non-existent workflow
        let non_existent_workflow = WorkflowID::new("does-not-exist").unwrap();
        let trigger_error = set_ethereum_trigger(
            file_path.clone(),
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
            file_path.clone(),
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
            file_path.clone(),
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
            file_path.clone(),
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
            file_path.clone(),
            Some(workflow_id.clone()),
            component_id.clone(),
            Some(1000),
        )
        .unwrap();

        // Initial workflow should have None submit (default when created)
        let service_initial: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let initial_workflow = service_initial.workflows.get(&workflow_id).unwrap();
        assert!(matches!(initial_workflow.submit, Submit::None));

        // Test setting Ethereum submit
        let eth_address = "0x00000000219ab540356cBB839Cbe05303d7705Fa".to_string();
        let eth_chain = ChainName::from_str("ethereum-mainnet").unwrap();
        let max_gas = Some(1000000u64);

        let eth_result = set_ethereum_submit(
            file_path.clone(),
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
        let service_after_eth: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        let eth_workflow = service_after_eth.workflows.get(&workflow_id).unwrap();
        if let Submit::EthereumContract {
            address,
            chain_name,
            max_gas: result_max_gas,
        } = &eth_workflow.submit
        {
            assert_eq!(address.to_string(), eth_address);
            assert_eq!(chain_name, &eth_chain);
            assert_eq!(result_max_gas, &max_gas);
        } else {
            panic!("Expected EthServiceHandler submit in service");
        }

        // Test updating with null max_gas
        let eth_result_no_gas = set_ethereum_submit(
            file_path.clone(),
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
            file_path.clone(),
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
            file_path.clone(),
            workflow_id.clone(),
            invalid_eth_address,
            eth_chain.clone(),
            max_gas,
        );
        assert!(invalid_eth_result.is_err());
        let invalid_address_error = invalid_eth_result.unwrap_err().to_string();
        assert!(invalid_address_error.contains("invalid"));
    }
}
