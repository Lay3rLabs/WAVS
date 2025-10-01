use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use utils::{
    config::{ChainConfigs, ConfigExt},
    service::DEFAULT_IPFS_GATEWAY,
};
use utoipa::ToSchema;
use wavs_types::{Credential, Workflow};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheckMode {
    /// Skip health checks, spawn background task to log results
    Bypass,
    /// Run health checks before startup, warn on failures (default)
    Wait,
    /// Run health checks before startup, panic on failures
    Exit,
}

impl Default for HealthCheckMode {
    fn default() -> Self {
        Self::Wait
    }
}

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Config {
    /// The port to bind the server to.
    /// Default is `8000`
    pub port: u32,
    /// The log-level to use, in the format of [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives).
    /// Default is `["info"]`
    pub log_level: Vec<String>,
    /// The host to bind the server to
    /// Default is `127.0.0.1`
    pub host: String,
    /// The directory to store all internal data files
    /// Default is `/var/wavs`
    #[schema(value_type = String)]
    pub data: PathBuf,
    /// The allowed cors origins
    /// Default is empty
    pub cors_allowed_origins: Vec<String>,

    // wasm engine config
    pub wasm_lru_size: usize,

    /// All the available chains
    pub chains: ChainConfigs,

    /// The mnemonic to use for submitting transactions on EVM chains
    pub submission_mnemonic: Option<Credential>,

    /// The mnemonic to use for submitting transactions on Cosmos chains
    pub cosmos_submission_mnemonic: Option<Credential>,

    /// The maximum amount of fuel (compute metering) to allow for 1 component's execution
    pub max_wasm_fuel: u64,

    /// The maximum amount of time (seconds) to allow for 1 component's execution
    pub max_execution_seconds: u64,

    /// Jaeger collector to send trace data
    pub jaeger: Option<String>,

    /// Prometheus collector to send metrics data
    pub prometheus: Option<String>,

    /// Prometheus metrics push interval in seconds (default: 30)
    pub prometheus_push_interval_secs: Option<u64>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    pub ipfs_gateway: String,

    /// Optional bearer token to protect mutating HTTP endpoints.
    /// If None, endpoints remain unauthenticated.
    pub bearer_token: Option<Credential>,

    /// Enable dev endpoints for testing (default: false)
    pub dev_endpoints_enabled: bool,

    /// Maximum HTTP request body size in megabytes (default: 15MB)
    pub max_body_size_mb: u32,

    /// Health check mode for chain endpoints at startup
    pub health_check_mode: HealthCheckMode,

    /// Disable trigger networking for testing (default: false)
    #[cfg(feature = "dev")]
    pub disable_trigger_networking: bool,

    /// Disable submission networking for testing (default: false)
    #[cfg(feature = "dev")]
    pub disable_submission_networking: bool,
}

impl ConfigExt for Config {
    fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
        f(&mut self.data);
    }

    fn log_levels(&self) -> impl Iterator<Item = &str> {
        self.log_level.iter().map(|s| s.as_str())
    }
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8000,
            log_level: vec!["info".to_string()],
            host: "127.0.0.1".to_string(),
            data: PathBuf::from("/var/wavs"),
            cors_allowed_origins: Vec::new(),
            chains: ChainConfigs {
                cosmos: BTreeMap::new(),
                evm: BTreeMap::new(),
                dev: BTreeMap::new(),
            },
            wasm_lru_size: 20,
            submission_mnemonic: None,
            cosmos_submission_mnemonic: None,
            max_execution_seconds: Workflow::DEFAULT_TIME_LIMIT_SECONDS,
            max_wasm_fuel: Workflow::DEFAULT_FUEL_LIMIT,
            jaeger: None,
            prometheus: None,
            prometheus_push_interval_secs: None,
            ipfs_gateway: DEFAULT_IPFS_GATEWAY.to_string(),
            bearer_token: None,
            dev_endpoints_enabled: false,
            max_body_size_mb: 15,
            health_check_mode: HealthCheckMode::default(),
            #[cfg(feature = "dev")]
            disable_trigger_networking: false,
            #[cfg(feature = "dev")]
            disable_submission_networking: false,
        }
    }
}
