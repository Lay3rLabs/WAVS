use std::path::PathBuf;

use clap::{arg, Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use utils::{
    config::{CliEnvExt, ConfigBuilder},
    serde::deserialize_vec_string,
};
use wavs::apis::dispatcher::ServiceConfig;

use crate::config::Config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    DeployEigenCore {
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(long, default_value = "local")]
        chain: String,

        #[clap(flatten)]
        args: CliArgs,
    },

    DeployService {
        /// If set, will register as an operator for the service too
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        /// Path to the WASI component
        #[clap(long)]
        component: PathBuf,

        /// The kind of trigger to deploy
        #[clap(long, default_value_t = CliTriggerKind::SimpleEthContract)]
        trigger: CliTriggerKind,

        /// The chain to deploy the trigger on, if applicable
        #[clap(long, default_value = "local")]
        trigger_chain: Option<String>,

        /// if the trigger is a cosmos trigger, the optional code id to use to avoid a re-upload
        #[clap(long, default_value = None)]
        cosmos_trigger_code_id: Option<u64>,

        /// The kind of submit to deploy
        #[clap(long, default_value_t = CliSubmitKind::SimpleEthContract)]
        submit: CliSubmitKind,

        /// The chain to deploy the submit on, if applicable
        #[clap(long, default_value = "local")]
        submit_chain: Option<String>,

        #[clap(flatten)]
        args: CliArgs,

        #[clap(long, value_parser = |json: &str| serde_json::from_str::<ServiceConfig>(json).map_err(|e| format!("Failed to parse JSON: {}", e)))]
        service_config: Option<ServiceConfig>,
    },

    /// Adds a task to a service that was previously deployed via CLI (uses stored deploy info)
    AddTask {
        #[clap(long)]
        service_id: String,

        #[clap(long)]
        workflow_id: Option<String>,

        /// The payload data, hex-encoded
        #[clap(long)]
        input: String,

        /// Optional time to wait for a result, in milliseconds
        /// if none, will return immediately without showing the result
        #[clap(long, default_value = "10_000")]
        result_timeout_ms: Option<u64>,

        #[clap(flatten)]
        args: CliArgs,
    },

    Exec {
        /// Path to the WASI component
        /// The component must implement the eth-trigger-world WIT
        #[clap(long)]
        component: PathBuf,

        #[clap(flatten)]
        args: CliArgs,

        /// The payload data, hex-encoded.
        /// If preceded by a `@`, will be treated as a file path
        #[clap(long)]
        input: String,
    },
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize, ValueEnum)]
pub enum CliTriggerKind {
    SimpleEthContract,
    SimpleCosmosContract,
}

impl std::fmt::Display for CliTriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SimpleEthContract => write!(f, "simple-eth-contract"),
            Self::SimpleCosmosContract => write!(f, "simple-cosmos-contract"),
        }
    }
}

impl std::str::FromStr for CliTriggerKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple-eth-contract" => Ok(Self::SimpleEthContract),
            "simple-cosmos-contract" => Ok(Self::SimpleCosmosContract),
            _ => Err(format!("unknown trigger kind: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
pub enum CliSubmitKind {
    SimpleEthContract,
}

impl std::fmt::Display for CliSubmitKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SimpleEthContract => write!(f, "simple-eth-contract"),
        }
    }
}

impl std::str::FromStr for CliSubmitKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple-eth-contract" => Ok(Self::SimpleEthContract),
            _ => Err(format!("unknown submit kind: {}", s)),
        }
    }
}

impl Command {
    pub fn args(&self) -> CliArgs {
        let args = match self {
            Self::DeployEigenCore { args, .. } => args,
            Self::DeployService { args, .. } => args,
            Self::AddTask { args, .. } => args,
            Self::Exec { args, .. } => args,
        };

        args.clone()
    }

    pub fn config(&self) -> Config {
        ConfigBuilder::new(self.args()).build().unwrap()
    }
}

/// This struct is used for both args and environment variables
/// the basic idea is that every env var can be overriden by a cli arg
/// and these override the config file
/// env vars follow the pattern of WAVS_CLI_{UPPERCASE_ARG_NAME}
#[derive(Clone, Debug, Parser, Serialize, Deserialize, Default)]
#[command(version, about, long_about = None)]
#[serde(default)]
pub struct CliArgs {
    /// The home directory of the application, where the wavs.toml configuration file is stored
    /// if not provided, a series of default directories will be tried
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,

    /// The WAVS endpoint
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wavs_endpoint: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotenv: Option<PathBuf>,

    /// Log level in the format of comma-separated tracing directives.
    /// See example config file for more info
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub log_level: Vec<String>,

    /// The directory to store all internal data files
    /// See example config file for more info
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PathBuf>,

    /// eth mnemonic (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eth_mnemonic: Option<String>,

    /// cosmos mnemonic (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_mnemonic: Option<String>,
}

impl CliEnvExt for CliArgs {
    const ENV_VAR_PREFIX: &'static str = "WAVS_CLI";

    fn home_dir(&self) -> Option<PathBuf> {
        self.home.clone()
    }

    fn dotenv_path(&self) -> Option<PathBuf> {
        self.dotenv.clone()
    }
}
