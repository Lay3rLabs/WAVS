use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use utils::{
    config::{ChainConfigs, ConfigExt},
    service::DEFAULT_IPFS_GATEWAY,
};
use utoipa::ToSchema;

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

    /// Mnemonic or private key of the transaction wallet for evm (usually leave this as None in config file and cli args, rather override in env)
    pub credential: Option<String>,

    /// Mnemonic of the transaction wallet for cosmos (usually leave this as None in config file and cli args, rather override in env)
    pub cosmos_mnemonic: Option<String>,

    /// The hd index of the mnemonic to sign with
    pub hd_index: Option<u32>,

    /// Jaeger collector to send trace data
    pub jaeger: Option<String>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    pub ipfs_gateway: String,
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
            cosmos_mnemonic: None,
            hd_index: None,
            jaeger: None,
            chains: ChainConfigs {
                cosmos: Default::default(),
                evm: Default::default(),
            },
            ipfs_gateway: DEFAULT_IPFS_GATEWAY.to_string(),
        }
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
