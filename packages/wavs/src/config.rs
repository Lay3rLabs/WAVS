use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};
use utils::config::{AnyChainConfig, ChainConfigs, ConfigExt, SigningPoolConfig};
use wavs_types::ChainName;

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub data: PathBuf,
    /// The allowed cors origins
    /// Default is empty
    pub cors_allowed_origins: Vec<String>,

    // wasm engine config
    pub wasm_lru_size: usize,
    pub wasm_threads: usize,

    /// The active chain names to watch for triggers
    pub active_trigger_chains: Vec<ChainName>,

    /// All the available chains
    pub chains: ChainConfigs,

    /// The mnemonic to use for submitting transactions on Ethereum chains
    pub submission_mnemonic: Option<String>,

    /// Configuration for the eth submission clients
    pub submission_pool_config: SigningPoolConfig,

    /// The mnemonic to use for submitting transactions on Cosmos chains
    pub cosmos_submission_mnemonic: Option<String>,

    /// Domain to use for registries
    pub registry_domain: Option<String>,
}

impl ConfigExt for Config {
    const FILENAME: &'static str = "wavs.toml";

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
            active_trigger_chains: Vec::new(),
            chains: ChainConfigs {
                cosmos: BTreeMap::new(),
                eth: BTreeMap::new(),
            },
            wasm_lru_size: 20,
            wasm_threads: 4,
            submission_mnemonic: None,
            cosmos_submission_mnemonic: None,
            registry_domain: None,
            submission_pool_config: SigningPoolConfig::default(),
        }
    }
}

impl Config {
    pub fn active_trigger_chain_configs(&self) -> HashMap<ChainName, AnyChainConfig> {
        self.chains
            .cosmos
            .iter()
            .filter_map(|(chain_name, chain)| {
                if self.active_trigger_chains.contains(chain_name) {
                    Some((chain_name.clone(), chain.clone().into()))
                } else {
                    None
                }
            })
            .chain(self.chains.eth.iter().filter_map(|(chain_name, chain)| {
                if self.active_trigger_chains.contains(chain_name) {
                    Some((chain_name.clone(), chain.clone().into()))
                } else {
                    None
                }
            }))
            .collect()
    }
}
