use anyhow::{bail, Result};
use figment::Figment;
use serde::{Deserialize, Serialize};
use utils::eth_client::{EthClientBuilder, EthClientConfig, EthQueryClient, EthSigningClient};

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
    pub endpoint: String,

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
            endpoint: "ws://127.0.0.1:8545".to_string(),
            mnemonic: None,
        }
    }
}

impl Config {
    pub async fn signing_client(&self) -> Result<EthSigningClient> {
        let endpoint = self.endpoint.clone();
        let mnemonic = self.mnemonic.clone();
        let eth_client = EthClientConfig { endpoint, mnemonic };
        let signing_client = EthClientBuilder::new(eth_client).build_signing().await?;
        Ok(signing_client)
    }

    pub async fn query_client(&self) -> Result<EthQueryClient> {
        let endpoint = self.endpoint.clone();
        let mnemonic = None;
        let eth_client = EthClientConfig { endpoint, mnemonic };
        let query_client = EthClientBuilder::new(eth_client).build_query().await?;
        Ok(query_client)
    }
}

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder {
    pub cli_args: CliArgs,
}

impl ConfigBuilder {
    pub fn new(cli_args: CliArgs) -> Self {
        Self { cli_args }
    }

    // merges the cli and env vars
    // which has optional values, by default None (or empty)
    // and parses complex types from strings
    // and has some differences from CONFIG like `home`

    pub fn merge_cli_env_args(&self) -> Result<CliArgs> {
        let cli_args: CliArgs = Figment::new()
            .merge(figment::providers::Env::prefixed("AGGREGATOR_"))
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
            // TODO: toml config for aggregator
            // .merge(figment::providers::Toml::file(Self::filepath(
            //     &cli_env_args,
            // )?))
            .merge(figment::providers::Serialized::defaults(cli_env_args))
            .join(figment::providers::Serialized::defaults(Config::default()))
            .extract()?;

        Ok(config)
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
