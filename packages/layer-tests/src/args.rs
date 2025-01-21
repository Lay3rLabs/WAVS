use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};
use utils::{config::CliEnvExt, filesystem::workspace_path};

/// This struct is used for both args and environment variables
/// the basic idea is that every env var can be overriden by a cli arg
/// and these override the config file
/// env vars follow the pattern of WAVS_{UPPERCASE_ARG_NAME}
#[derive(Debug, Parser, Serialize, Deserialize, Default)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct TestArgs {
    /// Run some specific test
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolated: Option<String>,

    /// Run all tests
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all: Option<bool>,
}

impl CliEnvExt for TestArgs {
    const ENV_VAR_PREFIX: &'static str = "WAVS_LAYER_TESTS";

    fn home_dir(&self) -> Option<PathBuf> {
        Some(workspace_path().join("packages").join("layer-tests"))
    }

    fn dotenv_path(&self) -> Option<PathBuf> {
        Some(
            workspace_path()
                .join("packages")
                .join("layer-tests")
                .join(".env"),
        )
    }
}
