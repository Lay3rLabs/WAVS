use std::path::PathBuf;

use alloy::signers::local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use serde::{Deserialize, Serialize};
use utils::{
    error::EthClientError,
    eth_client::{EthClientBuilder, EthClientConfig, EthSigningClient},
};

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
    /// Default is `8001`
    pub port: u32,
    /// The log-level to use, in the format of [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives).
    /// Default is `["info"]`
    pub log_level: Vec<String>,
    /// The host to bind the server to
    /// Default is `localhost`
    pub host: String,
    /// The allowed cors origins
    /// Default is empty
    pub cors_allowed_origins: Vec<String>,

    /// The chosen chain name
    pub chain: String,

    /// Websocket eth endpoint
    pub ws_endpoint: String,

    /// Http eth endpoint
    pub http_endpoint: String,

    /// Mnemonic of the signer (usually leave this as None in config file and cli args, rather override in env)
    pub mnemonic: Option<String>,
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8001,
            log_level: vec!["info".to_string()],
            host: "localhost".to_string(),
            cors_allowed_origins: Vec::new(),
            chain: String::new(),
            ws_endpoint: "ws://127.0.0.1:8545".to_string(),
            http_endpoint: "http://127.0.0.1:8545".to_string(),
            mnemonic: None,
        }
    }
}

impl Config {
    pub async fn signing_client(&self) -> Result<EthSigningClient> {
        let mnemonic = self.mnemonic.clone();
        let eth_client = EthClientConfig {
            ws_endpoint: self.ws_endpoint.clone(),
            http_endpoint: self.http_endpoint.clone(),
            mnemonic,
        };
        let signing_client = EthClientBuilder::new(eth_client).build_signing().await?;
        Ok(signing_client)
    }

    pub fn signer(&self) -> Result<PrivateKeySigner> {
        let mnemonic = self
            .mnemonic
            .clone()
            .ok_or(EthClientError::MissingMnemonic)?;
        let signer = MnemonicBuilder::<English>::default()
            .phrase(mnemonic)
            .build()?;
        Ok(signer)
    }
}

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder {
    pub cli_args: CliArgs,
}

impl ConfigBuilder {
    pub const FILENAME: &'static str = "aggregator.toml";
    pub const DIRNAME: &'static str = "aggregator";
    pub const HIDDEN_DIRNAME: &'static str = ".aggregator";

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
        // this does not append the default "aggregator" subdirectory
        // instead, it is used as the direct home directory
        // i.e. the path in this case will be /foo/aggregator.toml
        if let Some(dir) = cli_env_args.home.clone() {
            dirs.push(dir);
        }

        // next, check the current working directory, wherever the command is run from
        // i.e. ./aggregator.toml
        if let Ok(dir) = std::env::current_dir() {
            dirs.push(dir);
        }

        // here we want to check the user's home directory directly, not in the `.config` subdirectory
        // in this case, to not pollute the home directory, it looks for ~/.aggregator/aggregator.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(Self::HIDDEN_DIRNAME)) {
            dirs.push(dir);
        }

        // checks the `.aggregator/aggregator.toml` file in the system config directory
        // this will vary, but the final path with then be something like:
        // Linux: ~/.config/aggregator/aggregator.toml
        // macOS: ~/Library/Application Support/aggregator/aggregator.toml
        // Windows: C:\Users\MyUserName\AppData\Roaming\aggregator\aggregator.toml
        if let Some(dir) = dirs::config_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // On linux, this may already be added via config_dir above
        // but on macOS and windows, and maybe unix-like environments (msys, wsl, etc)
        // it's helpful to add it explicitly
        // the final path here typically becomes something like ~/.config/aggregator/aggregator.toml
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
        // this will definitively become something like ~/.config/aggregator/aggregator.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(".config").join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // Lastly, try /etc/aggregator/aggregator.toml
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
