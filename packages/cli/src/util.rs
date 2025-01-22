use anyhow::{Context, Result};
use std::path::Path;

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

                    if let Ok(bytes) = hex::decode(input) {
                        Ok(bytes)
                    } else {
                        let hex = input.as_bytes().iter().fold(String::new(), |mut acc, b| {
                            acc.push_str(&format!("{:02x}", b));
                            acc
                        });
                        hex::decode(hex).context("Failed to decode input")
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

pub fn read_component(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        // if relative path, parent (root of the repo) is relative 2 back from this file
        Path::new("../../").join(path.as_ref())
    };

    Ok(std::fs::read(path)?)
}
