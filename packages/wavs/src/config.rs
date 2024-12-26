use anyhow::{anyhow, bail, Context, Result};
use figment::{providers::Format, Figment};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use utils::eth_client::EthClientConfig;

use crate::args::{CliArgs, OptionalWavsChainConfig};

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
    /// Default is `localhost`
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
    pub chain: Option<String>,

    /// The chosen cosmos chain name
    pub cosmos_chain: Option<String>,

    /// All the available chains
    pub chains: ChainConfigs,

    #[serde(flatten)]
    pub chain_config_override: OptionalWavsChainConfig,
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8000,
            log_level: vec!["info".to_string()],
            host: "localhost".to_string(),
            data: PathBuf::from("/var/wavs"),
            cors_allowed_origins: Vec::new(),
            cosmos_chain: None,
            chain: None,
            chains: ChainConfigs {
                cosmos: HashMap::new(),
                eth: HashMap::new(),
            },
            chain_config_override: OptionalWavsChainConfig::default(),
            wasm_lru_size: 20,
            wasm_threads: 4,
        }
    }
}

impl Config {
    pub fn cosmos_chain_config(&self) -> Result<CosmosChainConfig> {
        let chain_name = self.cosmos_chain.as_deref();
        self.try_cosmos_chain_config()?.ok_or(anyhow!(
            "No chain config found for cosmos chain \"{}\"",
            chain_name.unwrap_or_default()
        ))
    }
    pub fn try_cosmos_chain_config(&self) -> Result<Option<CosmosChainConfig>> {
        let chain_name = self.cosmos_chain.as_deref();

        let config = match chain_name.and_then(|chain_name| self.chains.cosmos.get(chain_name)) {
            None => return Ok(None),
            Some(config) => config,
        };

        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // since in the CLI they get flattened and would conflict without a prefix
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix so that the fields match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct ConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub chain_id: Option<ChainId>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub bech32_prefix: Option<ChainId>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rpc_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub grpc_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub gas_price: Option<f32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub gas_denom: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub faucet_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub submission_mnemonic: Option<String>,
        }

        let config_override = ConfigOverride {
            chain_id: self.chain_config_override.cosmos_chain_id.clone(),
            bech32_prefix: self.chain_config_override.cosmos_bech32_prefix.clone(),
            grpc_endpoint: self.chain_config_override.cosmos_grpc_endpoint.clone(),
            gas_price: self.chain_config_override.cosmos_gas_price,
            gas_denom: self.chain_config_override.cosmos_gas_denom.clone(),
            faucet_endpoint: self.chain_config_override.cosmos_faucet_endpoint.clone(),
            submission_mnemonic: self
                .chain_config_override
                .cosmos_submission_mnemonic
                .clone(),
            rpc_endpoint: self.chain_config_override.cosmos_rpc_endpoint.clone(),
        };

        let config_merged = Figment::new()
            .merge(figment::providers::Serialized::defaults(config))
            .merge(figment::providers::Serialized::defaults(config_override))
            .extract()?;

        Ok(Some(config_merged))
    }

    pub fn ethereum_chain_config(&self) -> Result<EthereumChainConfig> {
        let chain_name = self.chain.as_deref();
        self.try_ethereum_chain_config()?.ok_or(anyhow!(
            "No chain config found for ethereum \"{}\"",
            chain_name.unwrap_or_default()
        ))
    }
    pub fn try_ethereum_chain_config(&self) -> Result<Option<EthereumChainConfig>> {
        let chain_name = self.chain.as_deref();

        let config = match chain_name.and_then(|chain_name| self.chains.eth.get(chain_name)) {
            None => return Ok(None),
            Some(config) => config,
        };

        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // since in the CLI they get flattened and would conflict without a prefix
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix so that the fields match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct ConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub chain_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub http_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ws_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub aggregator_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub submission_mnemonic: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub faucet_endpoint: Option<String>,
        }

        let config_override = ConfigOverride {
            chain_id: self.chain_config_override.chain_id.clone(),
            http_endpoint: self.chain_config_override.http_endpoint.clone(),
            ws_endpoint: self.chain_config_override.ws_endpoint.clone(),
            aggregator_endpoint: self.chain_config_override.aggregator_endpoint.clone(),
            submission_mnemonic: self.chain_config_override.submission_mnemonic.clone(),
            faucet_endpoint: self.chain_config_override.faucet_endpoint.clone(),
        };

        let config_merged = Figment::new()
            .merge(figment::providers::Serialized::defaults(config))
            .merge(figment::providers::Serialized::defaults(config_override))
            .extract()?;

        Ok(Some(config_merged))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChainConfigs {
    /// Cosmos-style chains (including Layer-SDK)
    pub cosmos: HashMap<String, CosmosChainConfig>,
    /// Ethereum-style chains
    pub eth: HashMap<String, EthereumChainConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CosmosChainConfig {
    pub chain_id: String,
    pub bech32_prefix: String,
    pub rpc_endpoint: Option<String>,
    pub grpc_endpoint: Option<String>,
    pub gas_price: f32,
    pub gas_denom: String,
    pub faucet_endpoint: Option<String>,
    /// mnemonic for the submission client (usually leave this as None and override in env)
    pub submission_mnemonic: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EthereumChainConfig {
    pub chain_id: String,
    pub ws_endpoint: String,
    pub http_endpoint: String,
    pub aggregator_endpoint: Option<String>,
    pub faucet_endpoint: Option<String>,
    /// mnemonic for the submission client (usually leave this as None and override in env)
    pub submission_mnemonic: Option<String>,
}

impl From<CosmosChainConfig> for layer_climb::prelude::ChainConfig {
    fn from(config: CosmosChainConfig) -> Self {
        Self {
            chain_id: ChainId::new(config.chain_id),
            rpc_endpoint: config.rpc_endpoint,
            grpc_endpoint: config.grpc_endpoint,
            grpc_web_endpoint: None,
            gas_price: config.gas_price,
            gas_denom: config.gas_denom,
            address_kind: AddrKind::Cosmos {
                prefix: config.bech32_prefix,
            },
        }
    }
}

impl From<EthereumChainConfig> for EthClientConfig {
    fn from(config: EthereumChainConfig) -> Self {
        Self {
            ws_endpoint: Some(config.ws_endpoint),
            http_endpoint: config.http_endpoint,
            mnemonic: config.submission_mnemonic,
            hd_index: None,
            transport: None,
        }
    }
}

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder {
    pub cli_args: CliArgs,
}

impl ConfigBuilder {
    pub const FILENAME: &'static str = "wavs.toml";
    pub const DIRNAME: &'static str = "wavs";
    pub const HIDDEN_DIRNAME: &'static str = ".wavs";

    pub fn new(cli_args: CliArgs) -> Self {
        Self { cli_args }
    }

    // merges the cli and env vars
    // which has optional values, by default None (or empty)
    // and parses complex types from strings
    // and has some differences from CONFIG like `home`

    pub fn merge_cli_env_args(&self) -> Result<CliArgs> {
        let cli_args: CliArgs = Figment::new()
            .merge(figment::providers::Env::prefixed(&format!(
                "{}_",
                CliArgs::ENV_VAR_PREFIX
            )))
            .merge(figment::providers::Serialized::defaults(&self.cli_args))
            .extract()?;

        Ok(cli_args)
    }

    pub fn build(self) -> Result<Config> {
        // try to load dotenv first, since it may affect env vars for filepaths
        let dotenv_path = self
            .cli_args
            .dotenv
            .clone()
            .unwrap_or(std::env::current_dir()?.join(".env"));

        if dotenv_path.exists() {
            if let Err(e) = dotenvy::from_path(dotenv_path) {
                bail!("Error loading dotenv file: {}", e);
            }
        }

        let cli_env_args = self.merge_cli_env_args()?;

        // then, our final config, which can have more complex types with easier TOML-like syntax
        // and also fills in defaults for required values at the end
        let config: Config = Figment::new()
            .merge(figment::providers::Toml::file(Self::filepath(
                &cli_env_args,
            )?))
            .merge(figment::providers::Serialized::defaults(cli_env_args))
            .join(figment::providers::Serialized::defaults(Config::default()))
            .extract()?;

        Ok(Config {
            data: shellexpand::tilde(&config.data.to_string_lossy())
                .to_string()
                .into(),
            ..config
        })
    }

    /// finds the filepath through a series of fallbacks
    /// the argument is internally derived cli + env args
    pub fn filepath(cli_env_args: &CliArgs) -> Result<PathBuf> {
        let filepaths_to_try = Self::filepaths_to_try(cli_env_args);

        filepaths_to_try
            .iter()
            .find(|filename| filename.exists())
            .with_context(|| {
                format!(
                    "No config file found, try creating one of these: {:?}",
                    filepaths_to_try
                )
            })
            .cloned()
    }

    /// provides the list of filepaths to try for the config file
    /// the argument is internally from cli + env args
    pub fn filepaths_to_try(cli_env_args: &CliArgs) -> Vec<PathBuf> {
        // the paths returned will be tried in order of pushing
        let mut dirs = Vec::new();

        // explicit arg passed to the cli, e.g. --home /foo, or env var HOME="/foo"
        // this does not append the default "wavs" subdirectory
        // instead, it is used as the direct home directory
        // i.e. the path in this case will be /foo/wavs.toml
        if let Some(dir) = cli_env_args.home.clone() {
            dirs.push(dir);
        }

        // next, check the current working directory, wherever the command is run from
        // i.e. ./wavs.toml
        if let Ok(dir) = std::env::current_dir() {
            dirs.push(dir);
        }

        // here we want to check the user's home directory directly, not in the `.config` subdirectory
        // in this case, to not pollute the home directory, it looks for ~/.wavs/wavs.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(Self::HIDDEN_DIRNAME)) {
            dirs.push(dir);
        }

        // checks the `.wavs/wavs.toml` file in the system config directory
        // this will vary, but the final path with then be something like:
        // Linux: ~/.config/wavs/wavs.toml
        // macOS: ~/Library/Application Support/wavs/wavs.toml
        // Windows: C:\Users\MyUserName\AppData\Roaming\wavs\wavs.toml
        if let Some(dir) = dirs::config_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // On linux, this may already be added via config_dir above
        // but on macOS and windows, and maybe unix-like environments (msys, wsl, etc)
        // it's helpful to add it explicitly
        // the final path here typically becomes something like ~/.config/wavs/wavs.toml
        if let Some(dir) = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .map(|dir| dir.join(Self::DIRNAME))
        {
            dirs.push(dir);
        }

        // Similarly, `config_dir` above may have already added this
        // but on systems like Windows, it's helpful to add it explicitly
        // since the system may place the config dir in AppData/Roaming
        // but we want to check the user's home dir first
        // this will definitively become something like ~/.config/wavs/wavs.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(".config").join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // Lastly, try /etc/wavs/wavs.toml
        dirs.push(PathBuf::from("/etc").join(Self::DIRNAME));

        // now we have a list of directories to check, we need to add the filename to each
        dirs.into_iter()
            .map(|dir| dir.join(Self::FILENAME))
            .collect()
    }
}

impl Config {
    pub fn tracing_env_filter(&self) -> Result<tracing_subscriber::EnvFilter> {
        let mut filter = tracing_subscriber::EnvFilter::from_default_env();
        for directive in &self.log_level {
            match directive.parse() {
                Ok(directive) => filter = filter.add_directive(directive),
                Err(err) => bail!("{}: {}", err, directive),
            }
        }

        Ok(filter)
    }
}
