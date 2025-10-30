use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utils::{config::CliEnvExt, serde::deserialize_vec_string};
use wavs_types::Credential;

/// This struct is used for both args and environment variables
/// the basic idea is that every env var can be overriden by a cli arg
/// and these override the config file
/// env vars follow the pattern of WAVS_AGGREGATOR_{UPPERCASE_ARG_NAME}
#[derive(Debug, Parser, Serialize, Deserialize, Default)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct CliArgs {
    /// The home directory of the application, where the aggregator.toml configuration file is stored
    /// if not provided, a series of default directories will be tried
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotenv: Option<PathBuf>,

    /// The port to bind the server to.
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,

    /// Log level
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub log_level: Vec<String>,

    /// The directory to store all internal data files
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PathBuf>,

    /// The host to bind the server to
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,

    /// The allowed cors origins
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub cors_allowed_origins: Vec<String>,

    /// The chain to use for the application
    /// will load from the config file
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,

    /// Mnemonic or private key (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<Credential>,

    /// Mnemonic for cosmos chains
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_credential: Option<Credential>,

    /// hd index of the mnemonic to sign with
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hd_index: Option<u32>,

    /// Number of tasks before submitting transaction
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks_quorum: Option<u32>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipfs_gateway: Option<String>,

    /// Optional bearer token to protect mutating HTTP endpoints
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<Credential>,

    /// Maximum HTTP request body size in megabytes
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_size_mb: Option<u32>,

    /// Jaeger collector to send trace data
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jaeger: Option<String>,

    /// Prometheus collector to send metrics data
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prometheus: Option<String>,

    /// Prometheus metrics push interval in seconds
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prometheus_push_interval_secs: Option<u64>,

    /// Enable dev endpoints for testing
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_endpoints_enabled: Option<bool>,

    /// Disable all network operations (for testing)
    #[cfg(debug_assertions)]
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_networking: Option<bool>,
}

impl CliEnvExt for CliArgs {
    const ENV_VAR_PREFIX: &'static str = "WAVS_AGGREGATOR";
    const TOML_IDENTIFIER: &'static str = "aggregator";

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn dotenv_path(&self) -> Option<PathBuf> {
        self.dotenv.clone()
    }
}
