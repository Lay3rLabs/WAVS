use anyhow::Result;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use wavs_types::{
    Component, ComponentID, ComponentSource, Digest, Permissions, Service, ServiceConfig,
    ServiceID, ServiceStatus,
};

use crate::{
    args::{ComponentCommand, ServiceCommand},
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
                let result = add_component(&file, id, digest).await?;
                ctx.handle_display_result(result);
            }
            ComponentCommand::Delete { id } => {
                let result = delete_component(&file, id).await?;
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
pub async fn modify_service_file<P, F, Fut, R>(file_path: P, modifier: F) -> Result<R>
where
    P: AsRef<Path>,
    F: FnOnce(Service) -> Fut,
    Fut: std::future::Future<Output = Result<(Service, R)>>,
{
    let file_path = file_path.as_ref();

    // Read the service file
    let service_json = std::fs::read_to_string(file_path)?;

    // Parse the service JSON
    let service: Service = serde_json::from_str(&service_json)?;

    // Apply the modification and get the result
    let (updated_service, result) = modifier(service).await?;

    // Convert updated service to JSON
    let updated_service_json = serde_json::to_string_pretty(&updated_service)?;

    // Write the updated JSON back to file
    let mut file = File::create(file_path)?;
    file.write_all(updated_service_json.as_bytes())?;

    Ok(result)
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
pub async fn add_component(
    file_path: &PathBuf,
    id: Option<ComponentID>,
    digest: Digest,
) -> Result<ComponentAddResult> {
    modify_service_file(file_path, |mut service| async move {
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
    .await
}

/// Delete a component from a service
pub async fn delete_component(
    file_path: &PathBuf,
    component_id: ComponentID,
) -> Result<ComponentDeleteResult> {
    modify_service_file(file_path, |mut service| async move {
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
    .await
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;
    use tempfile::tempdir;
    use tokio::runtime::Runtime;

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
        // Create a runtime for executing async code in tests
        let rt = Runtime::new().unwrap();

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
        let add_result = rt
            .block_on(async {
                add_component(&file_path, Some(component_id.clone()), test_digest.clone()).await
            })
            .unwrap();

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
        let second_add_result = rt
            .block_on(async {
                add_component(
                    &file_path,
                    Some(second_component_id.clone()),
                    test_digest.clone(),
                )
                .await
            })
            .unwrap();

        // Verify second add result
        assert_eq!(second_add_result.component_id, second_component_id);
        assert_eq!(second_add_result.digest, test_digest);

        // Verify the second component was added
        let service_after_second_add: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(service_after_second_add
            .components
            .contains_key(&second_component_id));
        assert_eq!(service_after_second_add.components.len(), 2); // First + second component

        // Test adding third component with Digest source
        let third_component_id = ComponentID::new("component-789").unwrap();
        let third_add_result = rt
            .block_on(async {
                add_component(
                    &file_path,
                    Some(third_component_id.clone()),
                    test_digest.clone(),
                )
                .await
            })
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
        let delete_result = rt
            .block_on(async { delete_component(&file_path, component_id.clone()).await })
            .unwrap();

        // Verify delete result
        assert_eq!(delete_result.component_id, component_id);
        assert_eq!(delete_result.file_path, file_path);

        // Verify the file was modified by deleting the component
        let service_after_delete: Service =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert!(!service_after_delete.components.contains_key(&component_id));
        assert_eq!(service_after_delete.components.len(), 2); // One removed, two remaining

        // Test error handling for non-existent component
        let non_existent_id = ComponentID::new("does-not-exist").unwrap();
        let error_result =
            rt.block_on(async { delete_component(&file_path, non_existent_id.clone()).await });

        // Verify it returns an error with appropriate message
        assert!(error_result.is_err());
        let error_msg = error_result.unwrap_err().to_string();
        assert!(error_msg.contains(&non_existent_id.to_string()));
        assert!(error_msg.contains("not found"));
    }
}
