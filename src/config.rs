use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::args::CliArgs;

/// The config struct we use in the application
#[derive(Debug)]
pub struct Config {
    /// The port to bind the server to.
    /// Default is `8000`
    pub port: u32,
    /// The log-level to use, in the format of [tracing directives](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives).
    /// Default is `["info"]`
    pub log_level: Vec<String>,
}

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder {
    pub args: CliArgs,
}

/// No need for this to be public, it's an intermediate struct
/// for config file which may have optional values
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConfigFile {
    pub port: Option<u32>,
    pub log_level: Option<Vec<String>>,
}

/// Defaults for config values that are optional
pub mod defaults {
    pub const PORT: u32 = 8000;
    pub const LOG_LEVEL: [&str; 1] = ["info"];
}

impl ConfigBuilder {
    pub const FILENAME: &str = "wasmatic.toml";
    pub const DIRNAME: &str = ".wasmatic";
    pub const ENV_VAR_PREFIX: &str = "MATIC";

    pub fn new(args: CliArgs) -> Self {
        Self { args }
    }

    pub async fn build(self) -> Result<Config> {
        self.load_env()?;

        let config = tokio::fs::read_to_string(&self.filepath()?).await?;
        let mut config: ConfigFile = toml::from_str(&config)?;

        if let Some(port) = Self::env_var("PORT")
            .map(|port| port.parse::<u32>())
            .transpose()?
        {
            config.port = Some(port);
        }

        if let Some(log_level) = Self::env_var("LOG_LEVEL")
            .map(|filter| filter.split(',').map(|x| x.trim().to_string()).collect())
        {
            config.log_level = Some(log_level);
        }

        Ok(Config {
            port: config.port.unwrap_or(defaults::PORT),
            log_level: config
                .log_level
                .unwrap_or(defaults::LOG_LEVEL.iter().map(|x| x.to_string()).collect()),
        })
    }

    fn load_env(&self) -> Result<()> {
        // try to load dotenv first, since it may affect env vars for filepaths
        let dotenv_path = self
            .args
            .dotenv
            .clone()
            .unwrap_or(std::env::current_dir()?.join(".env"));

        if dotenv_path.exists() {
            if let Err(e) = dotenvy::from_path(dotenv_path) {
                bail!("Error loading dotenv file: {}", e);
            }
        }

        Ok(())
    }

    pub fn env_var(name: &str) -> Option<String> {
        std::env::var(format!("{}_{name}", Self::ENV_VAR_PREFIX)).ok()
    }

    pub fn filepath(&self) -> Result<PathBuf> {
        let filepaths_to_try = self.filepaths_to_try();

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

    pub fn filepaths_to_try(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Some(dir) = self.args.home_dir.clone() {
            dirs.push(dir);
        }

        if let Some(dir) = Self::env_var("HOME").map(PathBuf::from) {
            dirs.push(dir);
        }

        if let Some(dir) = dirs::config_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        if let Some(dir) = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .map(|dir| dir.join(Self::DIRNAME))
        {
            dirs.push(dir);
        }

        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(".config").join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        if let Some(dir) = std::env::current_dir()
            .ok()
            .map(|dir| dir.join(Self::DIRNAME))
        {
            dirs.push(dir);
        }

        dirs.into_iter()
            .map(|dir| dir.join(Self::FILENAME))
            .collect()
    }
}

impl Config {
    pub fn tracing_env_filter(&self) -> Result<tracing_subscriber::EnvFilter> {
        let mut filter = tracing_subscriber::EnvFilter::from_default_env();
        for directive in &self.log_level {
            filter = filter.add_directive(directive.parse()?);
        }

        Ok(filter)
    }
}
