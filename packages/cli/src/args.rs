use std::path::PathBuf;

use clap::{arg, Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use utils::{
    config::{CliEnvExt, ConfigBuilder},
    serde::deserialize_vec_string,
    types::ChainName,
};
use wavs::apis::dispatcher::ServiceConfig;

use crate::config::Config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Deploy the core Eigenlayer contracts and (optionally) register as an Operator
    DeployEigenCore {
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        chain: ChainName,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Deploy a submit contract for eigenlayer (a.k.a. Service Manager
    /// Uses core contracts that were previously deployed via the CLI
    /// Typically used for getting an address to pass to DeployService
    DeployEigenSubmit {
        /// The chain to deploy the submit on, if applicable
        #[clap(long, default_value = "local")]
        chain: String,

        /// The payload handler contract address
        #[clap(long)]
        payload_handler: String,

        /// If set, will register as an operator for the service too
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Deploy a full service and (optionally) register as an Operator on the Submit target
    /// Uses core contracts that were previously deployed via the CLI
    DeployService {
        /// If set, will register as an operator for the service too
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        /// Path to the WASI component
        #[clap(long)]
        component: PathBuf,

        /// The kind of trigger to deploy
        #[clap(long)]
        trigger: CliTriggerKind,

        /// The will be event name for cosmos triggers, hex-encoded event signature for eth triggers
        #[clap(long, required_if_eq_any([
            ("trigger", CliTriggerKind::EthContractEvent),
            ("trigger", CliTriggerKind::CosmosContractEvent)
        ]))]
        trigger_event_name: Option<String>,

        /// The address used for trigger contracts. If not supplied, will deploy fresh "example trigger" contract
        #[clap(long)]
        trigger_address: Option<String>,

        /// The address used for the submit manager. If not supplied, will deploy fresh "example submit" contract
        #[clap(long)]
        submit_address: Option<String>,

        /// The chain to deploy the trigger on, if applicable
        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        trigger_chain: Option<ChainName>,

        /// if the trigger is a cosmos trigger, the optional code id to use to avoid a re-upload
        #[clap(long, default_value = None)]
        cosmos_trigger_code_id: Option<u64>,

        /// The kind of submit to deploy
        #[clap(long, default_value_t = CliSubmitKind::SimpleEthContract)]
        submit: CliSubmitKind,

        /// The chain to deploy the submit on, if applicable
        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        submit_chain: Option<ChainName>,

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
        #[clap(long, default_value = "10000")]
        result_timeout_ms: u64,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Execute a component directly, without going through WAVS
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

fn parse_chain_name(s: &str) -> Result<ChainName, String> {
    ChainName::try_from(s).map_err(|e| e.to_string())
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize, ValueEnum)]
pub enum CliTriggerKind {
    EthContractEvent,
    CosmosContractEvent,
}

impl std::fmt::Display for CliTriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EthContractEvent => write!(f, "eth-contract-event"),
            Self::CosmosContractEvent => write!(f, "cosmos-contract-event"),
        }
    }
}

impl std::str::FromStr for CliTriggerKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eth-contract-event" => Ok(Self::EthContractEvent),
            "cosmos-contract-event" => Ok(Self::CosmosContractEvent),
            _ => Err(format!("unknown trigger kind: {}", s)),
        }
    }
}

impl From<CliTriggerKind> for clap::builder::OsStr {
    fn from(trigger: CliTriggerKind) -> clap::builder::OsStr {
        trigger.to_string().into()
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
            Self::DeployEigenSubmit { args, .. } => args,
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
    /// The home directory of the application, where the cli.toml configuration file is stored
    /// if not provided here or in an env var, a series of default directories will be tried
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,

    /// The WAVS endpoint. Default is `http://127.0.0.1:8000`
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wavs_endpoint: Option<PathBuf>,

    /// The path to an optional dotenv file to try and load
    /// if not set, will be the current working directory's .env
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotenv: Option<PathBuf>,

    /// Log level in the format of comma-separated tracing directives.
    /// Default is "info"
    #[arg(long, value_delimiter = ',')]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "deserialize_vec_string")]
    pub log_level: Vec<String>,

    /// The directory to store all internal data files
    /// Default is /var/wavs-cli
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
