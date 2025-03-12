use anyhow::Result;
use std::{collections::BTreeMap, fs::File, io::Write, path::PathBuf};
use uuid::Uuid;
use wavs_types::{Service, ServiceConfig, ServiceID, ServiceStatus};

use crate::{args::ServiceCommand, context::CliContext};

/// Handle service commands - this function will be called from main.rs
pub fn handle_service_command(
    ctx: &CliContext,
    file: PathBuf,
    command: ServiceCommand,
) -> Result<()> {
    match command {
        ServiceCommand::Init { name, id } => {
            let result = init_service(file, name, id)?;
            ctx.handle_display_result(result);
        }
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

/// Run the service initialization
pub fn init_service(
    file_path: PathBuf,
    name: String,
    id: Option<String>,
) -> Result<ServiceInitResult> {
    // Generate service ID if not provided
    let id = ServiceID::new(match id {
        Some(id) => id,
        None => Uuid::now_v7().as_simple().to_string(),
    })?;

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
        let result = init_service(
            file_path.clone(),
            "Test Service".to_string(),
            Some("test-id-123".to_string()),
        )
        .unwrap();

        // Verify the result
        let expected = ServiceID::new("test-id-123").unwrap();
        assert_eq!(result.service.id, expected);
        assert_eq!(result.service.name, "Test Service");
        assert_eq!(result.file_path, file_path);

        // Verify the file was created
        assert!(file_path.exists());

        // Parse the created file to verify its contents
        let file_content = std::fs::read_to_string(file_path).unwrap();
        let parsed_service: Service = serde_json::from_str(&file_content).unwrap();

        assert_eq!(parsed_service.id, expected);
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
