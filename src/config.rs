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
    pub const ENV_VAR_PREFIX: &'static str = "MATIC";

    pub fn new(args: CliArgs) -> Self {
        Self { args }
    }

    pub async fn build(self) -> Result<Config> {
        self.load_env()?;

        // internal-only optional config struct used to hold values
        // for converting from cli/env vars to full config
        #[derive(Debug, Serialize, Deserialize)]
        struct OptionalConfig {
            #[serde(skip_serializing_if = "::std::option::Option::is_none")]
            pub port: Option<u32>,
            #[serde(skip_serializing_if = "::std::option::Option::is_none")]
            pub log_level: Option<Vec<String>>,
            #[serde(skip_serializing_if = "::std::option::Option::is_none")]
            pub host: Option<String>,
            #[serde(skip_serializing_if = "::std::option::Option::is_none")]
            pub data: Option<PathBuf>,
            #[serde(skip_serializing_if = "::std::option::Option::is_none")]
            pub cors_allowed_origins: Option<Vec<String>>,
        }

        impl From<&CliArgs> for OptionalConfig {
            fn from(args: &CliArgs) -> Self {
                fn parse_array_str(s: impl AsRef<str>) -> Vec<String> {
                    s.as_ref()
                        .split(',')
                        .map(|x| x.trim().to_string())
                        .collect()
                }

                Self {
                    port: args.port,
                    log_level: args.log_level.as_ref().map(parse_array_str),
                    host: args.host.clone(),
                    data: args.data.clone(),
                    cors_allowed_origins: args.cors_allowed_origins.as_ref().map(parse_array_str),
                }
            }
        }

        // not used directly, but rather to ensure we add all possible values
        impl From<&OptionalConfig> for Config {
            fn from(optional: &OptionalConfig) -> Self {
                Self {
                    port: optional.port.unwrap_or_default(),
                    log_level: optional.log_level.clone().unwrap_or_default(),
                    host: optional.host.clone().unwrap_or_default(),
                    data: optional.data.clone().unwrap_or_default(),
                    cors_allowed_origins: optional.cors_allowed_origins.clone().unwrap_or_default(),
                }
            }
        }

        // first parse env_config into cli_args (they use the same primitive types)
        // then convert to OptionalConfig so we can merge it into our real config
        let env_config = OptionalConfig::from(
            &Figment::new()
                .merge(figment::providers::Env::prefixed("MATIC_"))
                .extract()?,
        );

        // load cli args into a struct we can merge into the config
        let cli_config = OptionalConfig::from(&self.args);

        let config: Config = Figment::new()
            .merge(figment::providers::Toml::file(self.filepath()?))
            .merge(figment::providers::Serialized::defaults(env_config))
            .merge(figment::providers::Serialized::defaults(cli_config))
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
