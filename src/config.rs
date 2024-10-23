use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::args::CliArgs;

/// The config struct we use in the application
#[derive(Debug)]
pub struct Config {
    /// The port to bind the server to. If unspecified, will be 8000
    pub port: u32,
    /// The tracing filter to use. If unspecified, will be ["info"]
    pub tracing_filter: Vec<String>,
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
    pub tracing_filter: Option<Vec<String>>,
}

/// Defaults for config values that are optional
pub mod defaults {
    pub const PORT: u32 = 8000;
    pub const TRACING_FILTER: [&str; 1] = ["info"];
}

impl ConfigBuilder {
    pub const FILENAME: &str = "wasmatic.toml";
    pub const DIRNAME: &str = ".wasmatic";
    pub const ENV_VAR_PREFIX: &str = "MATIC";

    pub fn new(args: CliArgs) -> Self {
        Self { args }
    }

    pub async fn build(&self) -> Result<Config> {
        // try to load dotenv first, since it may affect env vars for filepaths
        let dotenv_path = self
            .args
            .dotenv
            .clone()
            .unwrap_or(std::env::current_dir()?.join(".env"));
        if dotenv_path.exists() {
            match dotenvy::from_path(dotenv_path) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error loading dotenv file: {}, continuing anyway", e);
                }
            }
        }

        let config = tokio::fs::read_to_string(&self.filepath()?).await?;
        let mut config: ConfigFile = toml::from_str(&config)?;

        if let Some(port) = Self::env_var("PORT")
            .map(|port| port.parse::<u32>())
            .transpose()?
        {
            config.port = Some(port);
        }

        if let Some(tracing_filter) = Self::env_var("TRACING_FILTER")
            .map(|filter| filter.split(',').map(|x| x.trim().to_string()).collect())
        {
            config.tracing_filter = Some(tracing_filter);
        }

        Ok(Config {
            port: config.port.unwrap_or(defaults::PORT),
            tracing_filter: config.tracing_filter.unwrap_or(
                defaults::TRACING_FILTER
                    .iter()
                    .map(|x| x.to_string())
                    .collect(),
            ),
        })
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
    pub fn build_tracing_filter(&self) -> Result<tracing_subscriber::EnvFilter> {
        let mut filter = tracing_subscriber::EnvFilter::from_default_env();
        for directive in &self.tracing_filter {
            filter = filter.add_directive(directive.parse()?);
        }

        Ok(filter)
    }
}
