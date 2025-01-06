use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use utils::config::{ChainConfigs, ConfigExt, CosmosChainConfig, EthereumChainConfig};

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

    /// The chosen ethereum chain name
    pub eth_chains: Vec<String>,

    /// The chosen cosmos chain name
    pub cosmos_chain: Option<String>,

    /// All the available chains
    pub chains: ChainConfigs,

    /// The mnemonic to use for submitting transactions on Ethereum chains
    pub submission_mnemonic: Option<String>,

    /// The mnemonic to use for submitting transactions on Cosmos chains
    pub cosmos_submission_mnemonic: Option<String>,

    /// The maximum amount of compute metering to allow for a single component execution
    /// Default is `1_000_000`
    pub max_wasm_fuel: u64,
}

impl ConfigExt for Config {
    const DIRNAME: &'static str = "wavs";
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
            cosmos_chain: None,
            eth_chains: Vec::new(),
            chains: ChainConfigs {
                cosmos: HashMap::new(),
                eth: HashMap::new(),
            },
            wasm_lru_size: 20,
            wasm_threads: 4,
            submission_mnemonic: None,
            cosmos_submission_mnemonic: None,
            max_wasm_fuel: 1_000_000,
        }
    }
}

impl Config {
    pub fn active_ethereum_chain_configs(&self) -> HashMap<String, EthereumChainConfig> {
        self.chains
            .eth
            .iter()
            .filter_map(|(chain_name, chain)| {
                if self.eth_chains.contains(chain_name) {
                    Some((chain_name.clone(), chain.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn cosmos_chain_config(&self) -> Result<&CosmosChainConfig> {
        match self.cosmos_chain.as_deref() {
            Some(chain_name) => self.chains.cosmos.get(chain_name).ok_or(anyhow::anyhow!(
                "No cosmos chain config found for chain: {}",
                chain_name
            )),
            None => bail!("No cosmos chain specified in config"),
        }
    }

    pub fn try_cosmos_chain_config(&self) -> Result<Option<&CosmosChainConfig>> {
        match self.cosmos_chain.as_deref() {
            Some(chain_name) => Ok(self.chains.cosmos.get(chain_name)),
            None => Ok(None),
        }
    }
}
