use anyhow::Result;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use wavs_types::{
    AllowedHostPermission, Component, ComponentID, ComponentSource, Digest, Permissions, Service,
    ServiceConfig, ServiceID, ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

use crate::{
    args::{ComponentCommand, ServiceCommand, WorkflowCommand},
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;
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
}
