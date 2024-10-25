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

        // first merge the cli and env vars
        // which has optional values, by default None (or empty)
        // and parses complex types from strings
        let cli_args: CliArgs = Figment::new()
            .merge(figment::providers::Env::prefixed("MATIC_"))
            .merge(figment::providers::Serialized::defaults(&self.args))
            .extract()?;

        // then, our final config, which can have more complex types with easier TOML-like syntax
        // and also fills in defaults for required values at the end
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

    /// provides the list of filepaths to try for the config file
    pub fn filepaths_to_try(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // explicit arg passed to the cli, e.g. --home /foo
        // this does not append the default ".wasmatic" subdirectory
        // instead, it is used as the direct home directory
        // the final path will then be /foo/wasmatic.toml
        if let Some(dir) = self.args.home.clone() {
            dirs.push(dir);
        }

        // similar to the cli argument, but instead the environment var set like MATIC_HOME="/foo"
        // this must be added separately from cli arg because at this point cli and env vars are not merged
        // the final path will then be /foo/wasmatic.toml
        if let Some(dir) = CliArgs::env_var("HOME") {
            dirs.push(dir.into());
        }

        // checks the `.wasmatic/wasmatic.toml` file in the system config directory
        // this will vary, but the final path with then be something like:
        // Linux: ~/.config/.wasmatic/wasmatic.toml
        // macOS: ~/Library/Application Support/.wasmatic/wasmatic.toml
        // Windows: C:\Users\MyUserName\AppData\Roaming\.wasmatic\wasmatic.toml
        if let Some(dir) = dirs::config_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // On linux, this may already be added via config_dir above
        // but on macOS and windows, and maybe unix-like environments (msys, wsl, etc)
        // it's helpful to add it explicitly
        // the final path here typically becomes something like ~/.config/.wasmatic/wasmatic.toml
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
        // this will definitively become something like ~/.config/.wasmatic/wasmatic.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(".config").join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // here we want to check the user's home directory directly, not in the `.config` subdirectory
        // the final path will be something like ~/.wasmatic/wasmatic.toml
        if let Some(dir) = dirs::home_dir().map(|dir| dir.join(Self::DIRNAME)) {
            dirs.push(dir);
        }

        // finally, check the current working directory, wherever the command is run from
        // i.e. ./wasmatic.toml
        if let Some(dir) = std::env::current_dir()
            .ok()
            .map(|dir| dir.join(Self::DIRNAME))
        {
            dirs.push(dir);
        }

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
            filter = filter.add_directive(directive.parse()?);
        }

        Ok(filter)
    }
}
