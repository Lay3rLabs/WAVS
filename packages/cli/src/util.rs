use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use utils::filesystem::workspace_path;

use crate::args::Component;

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
        pub fn read_component(component: &Component) -> Result<Vec<u8>> {
            match component {
                Component::Path(path) => {
                    let mut path = PathBuf::from(shellexpand::tilde(&path).to_string());

                    if !path.is_absolute() {
                        path = workspace_path().join(path)
                    };

                    Ok(std::fs::read(path)?)
                },
                Component::Registry(registry) => {
                    serde_json::to_vec(registry).map_err(|e| anyhow!(e))
                },
            }
        }
    } else {
        pub fn read_component(component: &Component) -> Result<Vec<u8>> {
            match component {
                Component::Path(path) => {
                    let path = PathBuf::from(shellexpand::tilde(&path).to_string());
                    Ok(std::fs::read(path)?)
                }
                Component::Registry(registry) => {
                    serde_json::to_vec(registry).map_err(|e| anyhow!(e))
                },
            }
        }
    }
}
