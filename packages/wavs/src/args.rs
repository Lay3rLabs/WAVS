use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utils::{config::CliEnvExt, serde::deserialize_vec_string};
use wavs_types::Credential;

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

    /// Log level
    #[arg(long)]
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
    #[arg(long)]
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
    /// signing keys are _derived_ from this using monotonic HD index
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_mnemonic: Option<Credential>,

    /// Optional aggregator credential for submitting to cosmos chains
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregator_cosmos_credential: Option<Credential>,

    /// Optional aggregator credential for submitting to evm chains
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregator_evm_credential: Option<Credential>,

    /// The maximum amount of fuel (compute metering) to allow for 1 component's execution
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_wasm_fuel: Option<u64>,

    /// The maximum amount of time (seconds) to allow for 1 component's execution
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_execution_seconds: Option<u64>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipfs_gateway: Option<String>,

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

    /// The interval in milliseconds to poll after submitting a transaction
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_poll_interval_ms: Option<u64>,

    /// Optional bearer token to protect mutating HTTP endpoints
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<Credential>,

    /// Maximum HTTP request body size in megabytes
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_size_mb: Option<u32>,

    /// Enable dev endpoints
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_endpoints_enabled: Option<bool>,

    /// Disable trigger networking for testing (default: false)
    #[cfg(feature = "dev")]
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_trigger_networking: Option<bool>,

    /// Disable submission networking for testing (default: false)
    #[cfg(feature = "dev")]
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_submission_networking: Option<bool>,

    /// Jetstream WebSocket endpoint URL for ATProto events
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jetstream_endpoint: Option<String>,

    /// Maximum jetstream message size in bytes
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jetstream_max_message_size: Option<usize>,
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
