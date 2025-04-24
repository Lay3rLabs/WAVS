use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utils::{config::CliEnvExt, serde::deserialize_vec_string};

/// This struct is used for both args and environment variables
/// the basic idea is that every env var can be overriden by a cli arg
/// and these override the config file
/// env vars follow the pattern of WAVS_{UPPERCASE_ARG_NAME}
#[derive(Debug, Parser, Serialize, Deserialize, Default)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct CliArgs {
    /// The home directory of the application, where the wavs.toml configuration file is stored
    /// if not provided here or in an env var, a series of default directories will be tried
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotenv: Option<PathBuf>,

    /// The port to bind the server to.
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,

    /// Log level in the format of comma-separated tracing directives.
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub log_level: Vec<String>,

    /// The host to bind the server to
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// The directory to store all internal data files
    /// Default is /var/wavs
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PathBuf>,

    /// The allowed cors origins
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub cors_allowed_origins: Vec<String>,

    /// Size of the LRU cache for in-memory components
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_lru_size: Option<usize>,

    /// Number of threads to run WASI components on
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_threads: Option<usize>,

    /// mnemonic for the submission client (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_mnemonic: Option<String>,

    /// mnemonic for the submission client (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_submission_mnemonic: Option<String>,

    /// The maximum amount of fuel (compute metering) to allow for 1 component's execution
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_wasm_fuel: Option<u64>,

    /// The maximum amount of time (seconds) to allow for 1 component's execution
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_execution_seconds: Option<u64>,

    /// The Pinata JWT for uploading to IPFS
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinata_jwt: Option<String>,
}

impl CliEnvExt for CliArgs {
    const ENV_VAR_PREFIX: &'static str = "WAVS";
    const TOML_IDENTIFIER: &'static str = "wavs";

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn dotenv_path(&self) -> Option<PathBuf> {
        self.dotenv.clone()
    }
}
