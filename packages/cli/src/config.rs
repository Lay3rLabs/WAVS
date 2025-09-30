use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};
use utils::{
    config::{ChainConfigs, ConfigExt},
    service::DEFAULT_IPFS_GATEWAY,
};
use wavs_types::Credential;

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// The port to bind the server to.
    /// Default is `http://127.0.0.1:8000`
    pub wavs_endpoint: String,
    /// The log-level to use, in the format of [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives).
    /// Default is `["info"]`
    pub log_level: Vec<String>,
    /// The directory to store all internal data files
    /// Default is `/var/wavs-cli`
    pub data: PathBuf,

    /// All the available chains
    pub chains: Arc<RwLock<ChainConfigs>>,

    /// The mnemonic to use for submitting transactions on cosmos chains (usually None, set via env var)
    pub cosmos_mnemonic: Option<Credential>,

    /// The credential to use for submitting transactions on evm chains (mnemonic or private key)
    pub evm_credential: Option<Credential>,

    /// The IPFS gateway URL used to access IPFS content over HTTP.
    pub ipfs_gateway: String,
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
            wavs_endpoint: "http://127.0.0.1:8000".to_string(),
            log_level: vec!["info".to_string()],
            data: PathBuf::from("/var/wavs-cli"),
            chains: Arc::new(RwLock::new(ChainConfigs::default())),
            cosmos_mnemonic: None,
            evm_credential: None,
            ipfs_gateway: DEFAULT_IPFS_GATEWAY.to_string(),
        }
    }
}
