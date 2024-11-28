use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

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

    /// The chosen layer chain name
    pub layer_chain: Option<String>,

    /// The chosen ethereum chain name
    pub chain: Option<String>,

    /// A lookup of chain configs, keyed by a "chain name"
    pub chains: HashMap<String, WavsChainConfig>,

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
            layer_chain: None,
            chain: None,
            chains: HashMap::new(),
            chain_config_override: OptionalWavsChainConfig::default(),
            wasm_lru_size: 20,
            wasm_threads: 4,
        }
    }
}

impl Config {
    pub fn layer_chain_config(&self) -> Result<WavsCosmosChainConfig> {
        let chain_name = self.layer_chain.as_deref();

        let config = chain_name
            .and_then(|chain_name| self.chains.get(chain_name))
            .context(format!(
                "No chain config found for \"{}\"",
                chain_name.unwrap_or_default()
            ))?;

        let config = match config {
            WavsChainConfig::Cosmos(config) => config,
            WavsChainConfig::Ethereum(_) => bail!("Expected Cosmos chain config, found Ethereum"),
        };

        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix in order for the fields to match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct ConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub chain_id: Option<ChainId>,
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
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rpc_endpoint: Option<String>,
        }

        let config_override = ConfigOverride {
            chain_id: self.chain_config_override.layer_chain_id.clone(),
            grpc_endpoint: self.chain_config_override.layer_grpc_endpoint.clone(),
            gas_price: self.chain_config_override.layer_gas_price,
            gas_denom: self.chain_config_override.layer_gas_denom.clone(),
            faucet_endpoint: self.chain_config_override.layer_faucet_endpoint.clone(),
            submission_mnemonic: self.chain_config_override.layer_submission_mnemonic.clone(),
            rpc_endpoint: self.chain_config_override.layer_rpc_endpoint.clone(),
        };

        let config_merged = Figment::new()
            .merge(figment::providers::Serialized::defaults(config))
            .merge(figment::providers::Serialized::defaults(config_override))
            .extract()?;

        Ok(config_merged)
    }

    pub fn ethereum_chain_config(&self) -> Result<WavsEthereumChainConfig> {
        let chain_name = self.chain.as_deref();

        let config = chain_name
            .and_then(|chain_name| self.chains.get(chain_name))
            .context(format!(
                "No chain config found for \"{}\"",
                chain_name.unwrap_or_default()
            ))?;

        let config = match config {
            WavsChainConfig::Cosmos(_) => bail!("Expected Ethereum chain config, found Cosmos"),
            WavsChainConfig::Ethereum(config) => config,
        };

        // The optional overrides use a prefix to distinguish between layer and ethereum fields
        // in order to cleanly merge it with our final, real chain config
        // we need to strip that prefix in order for the fields to match
        #[derive(Clone, Debug, Serialize, Deserialize, Default)]
        struct ConfigOverride {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rpc_endpoint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ws_endpoint: Option<String>,
        }

        let config_override = ConfigOverride {
            rpc_endpoint: self.chain_config_override.rpc_endpoint.clone(),
            ws_endpoint: self.chain_config_override.ws_endpoint.clone(),
        };

        let config_merged = Figment::new()
            .merge(figment::providers::Serialized::defaults(config))
            .merge(figment::providers::Serialized::defaults(config_override))
            .extract()?;

        Ok(config_merged)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "chain_kind")]
pub enum WavsChainConfig {
    Cosmos(WavsCosmosChainConfig),
    Ethereum(WavsEthereumChainConfig),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WavsCosmosChainConfig {
    pub chain_id: ChainId,
    pub rpc_endpoint: String,
    pub grpc_endpoint: String,
    /// not micro-units, e.g. 0.025 would be a typical value
    /// if not specified, defaults to 0.025
    #[serde(default = "WavsCosmosChainConfig::default_gas_price")]
    pub gas_price: f32,
    /// if not specified, defaults to "uslay"
    #[serde(default = "WavsCosmosChainConfig::default_gas_denom")]
    pub gas_denom: String,
    /// if not specified, defaults to "layer"
    #[serde(default = "WavsCosmosChainConfig::default_bech32_prefix")]
    pub bech32_prefix: String,
    /// optional faucet endpoint for this chain
    pub faucet_endpoint: Option<String>,
    /// mnemonic for the submission client (usually leave this as None and override in env)
    pub submission_mnemonic: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WavsEthereumChainConfig {
    pub ws_endpoint: String,
    pub rpc_endpoint: String,
}

impl WavsCosmosChainConfig {
    const fn default_gas_price() -> f32 {
        0.025
    }

    fn default_gas_denom() -> String {
        "uslay".to_string()
    }

    fn default_bech32_prefix() -> String {
        "layer".to_string()
    }
}

impl From<WavsCosmosChainConfig> for ChainConfig {
    fn from(config: WavsCosmosChainConfig) -> Self {
        Self {
            chain_id: config.chain_id,
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

        Ok(config)
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
