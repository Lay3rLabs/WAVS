use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
#[cfg(debug_assertions)]
use utils::filesystem::workspace_path;

//A wrapper type that will:
// 1. treat strings prefixed with @ as filepaths
// 2. treat strings prefixed with 0x as hex-encoded bytes
// 3. treat anything else as a string into raw bytes
pub struct ComponentInput(String);

impl ComponentInput {
    pub fn new(input: impl ToString) -> Self {
        Self(input.to_string())
    }

    pub fn decode(&self) -> Result<Vec<u8>> {
        let input = &self.0;
        match input.starts_with('@') {
            true => {
                let filepath = shellexpand::tilde(&input[1..]).to_string();

                Ok(std::fs::read(filepath)?)
            }

            false => {
                if Path::new(&shellexpand::tilde(&input).to_string()).exists() {
                    tracing::warn!("Are you sure you didn't mean to use @ to specify file input?");
                }

                match input.starts_with("0x") {
                    true => Ok(const_hex::decode(input)?),
                    false => Ok(input.as_bytes().to_vec()),
                }
            }
        }
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

cfg_if::cfg_if! {
    if #[cfg(debug_assertions)] {
        pub fn read_component(path: &str) -> Result<Vec<u8>> {
            let mut path = PathBuf::from(shellexpand::tilde(&path).to_string());
            if !path.is_absolute() {
                path = workspace_path().join(path)
            };
            Ok(std::fs::read(path)?)
        }
    } else {
        pub fn read_component(path: &str) -> Result<Vec<u8>> {
            let path = PathBuf::from(shellexpand::tilde(&path).to_string());
            Ok(std::fs::read(path)?)
        }
    }
}

/// Helper function to write serializable data to an output file
pub fn write_output_file<T: Serialize>(data: &T, path: &PathBuf) -> Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create directory {}: {}", parent.display(), e);
                std::process::exit(1);
            }
        }
    }

    // Serialize and write to file
    let json_output = serde_json::to_string(data)?;
    if let Err(e) = std::fs::write(path, json_output) {
        tracing::error!("Failed to write output to {}: {}", path.display(), e);
        std::process::exit(1);
    }

    tracing::info!("Output written to {}", path.display());
    Ok(())
}
