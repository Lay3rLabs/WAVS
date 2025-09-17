use std::path::PathBuf;

use alloy_signer_local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use utils::{
    config::{ChainConfigs, ConfigExt},
    service::DEFAULT_IPFS_GATEWAY,
};
use utoipa::ToSchema;
use wavs_types::Credential;

/// Default LRU cache size for compiled WASM components
const DEFAULT_WASM_LRU_SIZE: usize = 20;

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Config {
    /// The port to bind the server to.
    /// Default is `8001`
    pub port: u32,
    /// The log-level to use, in the format of [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives).
    /// Default is `["info"]`
    pub log_level: Vec<String>,
    /// The host to bind the server to
    /// Default is `127.0.0.1`
    pub host: String,
    /// The directory to store all internal data files
    /// Default is `/var/aggregator`
    #[schema(value_type = String)]
    pub data: PathBuf,
    /// The allowed cors origins
    /// Default is empty
    pub cors_allowed_origins: Vec<String>,

    /// All the available chains
    pub chains: ChainConfigs,

    /// Mnemonic or private key of the signer (usually leave this as None in config file and cli args, rather override in env)
    pub credential: Option<Credential>,

    /// The hd index of the mnemonic to sign with
    pub hd_index: Option<u32>,

    /// Jaeger collector to send trace data
    pub jaeger: Option<String>,

    /// Prometheus push gateway to send metrics
    pub prometheus: Option<String>,

    /// Prometheus metrics push interval in seconds
    pub prometheus_push_interval_secs: Option<u64>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    pub ipfs_gateway: String,

    /// LRU cache size for WASM components
    pub wasm_lru_size: usize,

    /// Maximum fuel for WASM execution (None for unlimited)
    pub max_wasm_fuel: Option<u64>,

    /// Maximum execution time in seconds for WASM components (None for unlimited)
    pub max_execution_seconds: Option<u64>,

    /// Optional bearer token to protect mutating HTTP endpoints.
    /// If None, endpoints remain unauthenticated.
    pub bearer_token: Option<Credential>,

    /// Enable dev endpoints for testing (default: false)
    pub dev_endpoints_enabled: bool,

    /// Maximum HTTP request body size in megabytes (default: 15MB)
    pub max_body_size_mb: u32,
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8001,
            log_level: vec!["info".to_string()],
            host: "127.0.0.1".to_string(),
            data: PathBuf::from("/var/aggregator"),
            cors_allowed_origins: Vec::new(),
            credential: None,
            hd_index: None,
            jaeger: None,
            prometheus: None,
            prometheus_push_interval_secs: None,
            chains: ChainConfigs {
                cosmos: Default::default(),
                evm: Default::default(),
                dev: Default::default(),
            },
            ipfs_gateway: DEFAULT_IPFS_GATEWAY.to_string(),
            wasm_lru_size: DEFAULT_WASM_LRU_SIZE,
            max_wasm_fuel: None,
            max_execution_seconds: None,
            bearer_token: None,
            dev_endpoints_enabled: false,
            max_body_size_mb: 15,
        }
    }
}

impl Config {
    pub fn signer(&self) -> Result<PrivateKeySigner> {
        let mnemonic = self
            .credential
            .clone()
            .ok_or(anyhow::anyhow!("missing credentials"))?;
        let signer = MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_ref())
            .build()?;
        Ok(signer)
    }
}

impl ConfigExt for Config {
    fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
        f(&mut self.data);
    }

    fn log_levels(&self) -> impl Iterator<Item = &str> {
        self.log_level.iter().map(|s| s.as_str())
    }
}
