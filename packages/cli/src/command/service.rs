use anyhow::Result;
use std::{collections::BTreeMap, fs::File, io::Write, path::PathBuf};
use uuid::Uuid;
use wavs_types::{
    Component, ComponentID, Digest, Permissions, Service, ServiceConfig, ServiceID, ServiceStatus,
};

use crate::{
    args::{ComponentCommand, ServiceCommand},
    clients::HttpClient,
    context::CliContext,
    util::read_component,
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
            ComponentCommand::Add { id, component } => {
                let result = add_component(ctx, file, id, component).await?;
                ctx.handle_display_result(result);
            }
            ComponentCommand::Delete { id } => {
                let result = delete_component(file, id)?;
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
    ctx: &CliContext,
    file_path: PathBuf,
    id: Option<ComponentID>,
    component_path: PathBuf,
) -> Result<ComponentAddResult> {
    // Read the service file
    let service_json = std::fs::read_to_string(&file_path)?;

    // Parse the service JSON
    let mut service: Service = serde_json::from_str(&service_json)?;

    // Generate component ID if not provided
    let component_id = match id {
        Some(id) => id,
        None => ComponentID::new(Uuid::now_v7().as_hyphenated().to_string())?,
    };

    // Upload the component
    let wasm_bytes = read_component(
        component_path
            .to_str()
            .expect("Invalid component path specified"),
    )?;
    let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());
    let digest = http_client.upload_component(wasm_bytes).await?;

    // Create a new component entry
    let component = Component {
        wasm: digest.clone(),
        permissions: Permissions::default(),
    };

    // Add the component to the service
    service.components.insert(component_id.clone(), component);

    // Convert updated service to JSON
    let updated_service_json = serde_json::to_string_pretty(&service)?;

    // Write the updated JSON back to file
    let mut file = File::create(&file_path)?;
    file.write_all(updated_service_json.as_bytes())?;

    Ok(ComponentAddResult {
        component_id,
        digest,
        file_path,
    })
}

/// Delete a component from a service
pub fn delete_component(
    file_path: PathBuf,
    component_id: ComponentID,
) -> Result<ComponentDeleteResult> {
    // Read the service file
    let service_json = std::fs::read_to_string(&file_path)?;

    // Parse the service JSON
    let mut service: Service = serde_json::from_str(&service_json)?;

    // Check if the component exists
    if !service.components.contains_key(&component_id) {
        return Err(anyhow::anyhow!(
            "Component with ID '{}' not found in service",
            component_id
        ));
    }

    // Remove the component
    service.components.remove(&component_id);

    // Convert updated service to JSON
    let updated_service_json = serde_json::to_string_pretty(&service)?;

    // Write the updated JSON back to file
    let mut file = File::create(&file_path)?;
    file.write_all(updated_service_json.as_bytes())?;

    Ok(ComponentDeleteResult {
        component_id,
        file_path,
    })
}

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn test_service_init_with_generated_id() {
        // Create a temporary directory for the test
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("generated_id_service.json");

        // Initialize service with no ID (should generate one)
        let result = init_service(file_path.clone(), "Auto ID Service".to_string(), None).unwrap();

        // Verify the result has a generated ID
        assert!(!result.service.id.is_empty());
        assert_eq!(result.service.name, "Auto ID Service");

        // Parse the created file to verify its contents
        let file_content = std::fs::read_to_string(file_path).unwrap();
        let parsed_service: Service = serde_json::from_str(&file_content).unwrap();

        assert!(!parsed_service.id.is_empty());
        assert_eq!(parsed_service.id, result.service.id);
    }
}
