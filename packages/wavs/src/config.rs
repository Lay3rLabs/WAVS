use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use utils::config::{ChainConfigs, CosmosChainConfig, OptionalWavsChainConfig};

use crate::args::CliArgs;

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
    pub eth_chains: Vec<String>,

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
            eth_chains: Vec::new(),
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
        let mut config: Config = Figment::new()
            .merge(figment::providers::Toml::file(Self::filepath(
                &cli_env_args,
            )?))
            .merge(figment::providers::Serialized::defaults(cli_env_args))
            .join(figment::providers::Serialized::defaults(Config::default()))
            .extract()?;

        config.chains = config
            .chains
            .merge_overrides(&config.chain_config_override)?;

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
