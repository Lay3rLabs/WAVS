use anyhow::{bail, Context, Result};
use figment::{providers::Format, Figment};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::args::CliArgs;

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize)]
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
    /// Default is `/var/wasmatic`
    pub data: PathBuf,
    /// The allowed cors origins
    /// Default is empty
    pub cors_allowed_origins: Vec<String>,
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8000,
            log_level: vec!["info".to_string()],
            host: "localhost".to_string(),
            data: PathBuf::from("/var/wasmatic"),
            cors_allowed_origins: Vec::new(),
        }
    }
}

/// The builder we use to build Config
#[derive(Debug)]
pub struct ConfigBuilder {
    pub args: CliArgs,
}

impl ConfigBuilder {
    pub const FILENAME: &'static str = "wasmatic.toml";
    pub const DIRNAME: &'static str = ".wasmatic";

    pub fn new(args: CliArgs) -> Self {
        Self { args }
    }

    pub async fn build(self) -> Result<Config> {
        self.load_env()?;

        let cli_args: CliArgs = Figment::new()
            .merge(figment::providers::Env::prefixed("MATIC_"))
            .merge(figment::providers::Serialized::defaults(&self.args))
            .extract()?;

        let config: Config = Figment::new()
            .merge(figment::providers::Toml::file(self.filepath()?))
            .merge(figment::providers::Serialized::defaults(cli_args))
            .join(figment::providers::Serialized::defaults(Config::default()))
            .extract()?;

        Ok(config)
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

        if let Some(dir) = self.args.home.clone() {
            dirs.push(dir);
        }

        // this must be added explicitly, because filepaths is derived
        // before we merge the cli and env vars
        if let Some(dir) = CliArgs::env_var("HOME") {
            dirs.push(dir.into());
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
