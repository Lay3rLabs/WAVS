use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use utils::filesystem::workspace_path;

pub enum ComponentInput {
    Stdin(String),
    Raw(Vec<u8>),
}

impl ComponentInput {
    pub fn decode(&self) -> Result<Vec<u8>> {
        match self {
            ComponentInput::Stdin(input) => match input.starts_with('@') {
                true => {
                    let filepath = shellexpand::tilde(&input[1..]).to_string();

                    Ok(std::fs::read(filepath)?)
                }

                false => {
                    if Path::new(&shellexpand::tilde(&input).to_string()).exists() {
                        tracing::warn!(
                            "Are you sure you didn't mean to use @ to specify file input?"
                        );
                    }

                    if let Ok(bytes) = const_hex::decode(input) {
                        Ok(bytes)
                    } else {
                        let hex = input.as_bytes().iter().fold(String::new(), |mut acc, b| {
                            acc.push_str(&format!("{:02x}", b));
                            acc
                        });
                        const_hex::decode(hex).context("Failed to decode input")
                    }
                }
            },
            ComponentInput::Raw(input) => Ok(input.clone()),
        }
    }

    pub fn into_string(self) -> String {
        match self {
            ComponentInput::Stdin(input) => input,
            ComponentInput::Raw(input) => String::from_utf8_lossy(&input).to_string(),
        }
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
