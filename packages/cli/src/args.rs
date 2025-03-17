use std::path::PathBuf;

use clap::{arg, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use utils::{
    config::{CliEnvExt, ConfigBuilder},
    serde::deserialize_vec_string,
};
use wavs_types::{ChainName, ComponentID, Service, ServiceConfig, ServiceID};

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

    /// Deploy a service manager contract for eigenlayer
    /// Uses core contracts that were previously deployed via the CLI
    /// Typically used for getting an address to pass to DeployEigenServiceHandler
    DeployEigenServiceManager {
        /// The chain to deploy the submit on, if applicable
        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        chain: ChainName,

        /// If set, will register as an operator for the service too
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Deploy a full service and (optionally) register as an Operator on the Submit target
    /// Uses core contracts that were previously deployed via the CLI
    DeployService {
        /// Path to the WASI component
        #[clap(long)]
        component: String,

        /// The kind of trigger to deploy
        /// If not set, will assume the trigger from the trigger_address
        #[clap(long)]
        trigger: Option<CliTriggerKind>,

        /// The will be event name for cosmos triggers, hex-encoded event signature for eth triggers
        #[clap(long, required_if_eq_any([
            ("trigger", CliTriggerKind::EthContractEvent),
            ("trigger", CliTriggerKind::CosmosContractEvent)
        ]))]
        trigger_event_name: Option<String>,

        /// The address used for trigger contracts, if applicable
        #[clap(long, required_if_eq_any([
            ("trigger", CliTriggerKind::EthContractEvent),
            ("trigger", CliTriggerKind::CosmosContractEvent)
        ]))]
        trigger_address: Option<String>,

        /// The kind of submit to deploy
        #[clap(long, default_value_t = CliSubmitKind::EthServiceHandler)]
        submit: CliSubmitKind,

        /// The address used for submit contracts, if applicable
        #[clap(long, required_if_eq_any([
            ("submit", CliSubmitKind::EthServiceHandler),
        ]))]
        #[clap(long)]
        submit_address: Option<String>,

        /// The chain to deploy the trigger on, if applicable
        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        trigger_chain: Option<ChainName>,

        /// The chain to deploy the submit on, if applicable
        #[clap(long, default_value = "local", value_parser = parse_chain_name)]
        submit_chain: Option<ChainName>,

        #[clap(flatten)]
        args: CliArgs,

        #[clap(long, value_parser = |json: &str| serde_json::from_str::<ServiceConfig>(json).map_err(|e| format!("Failed to parse JSON: {}", e)))]
        service_config: Option<ServiceConfig>,
    },

    /// Uploads a WASI component
    UploadComponent {
        /// Path to the WASI component
        #[clap(long)]
        component: String,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Deploy a service from a full JSON-encoded `Service`
    /// If the input is prefixed with `@`, it will be read from the file path
    /// Uses core contracts that were previously deployed via the CLI
    /// Assumes that the components have already been uploaded, operators have already registered on contracts
    DeployServiceRaw {
        #[clap(long, value_parser = parse_service_input)]
        service: Service,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Execute a component directly, without going through WAVS
    Exec {
        /// Path to the WASI component
        /// The component must implement the eth-trigger-world WIT
        #[clap(long)]
        component: String,

        #[clap(flatten)]
        args: CliArgs,

        /// The payload data.
        /// If preceded by a `@`, will be treated as a file path
        /// If preceded by a `0x`, will be treated as hex-encoded
        /// Otherwise will be treated as raw string bytes
        #[clap(long)]
        input: String,

        /// Optional service config
        #[clap(long, value_parser = |json: &str| serde_json::from_str::<ServiceConfig>(json).map_err(|e| format!("Failed to parse JSON: {}", e)))]
        service_config: Option<ServiceConfig>,

        /// Optional fuel limit for component execution
        #[clap(long)]
        fuel_limit: Option<u64>,
    },

    /// Service management commands
    Service {
        #[clap(subcommand)]
        command: ServiceCommand,

        /// Output file path
        #[clap(long, short, default_value = "./service.json")]
        file: PathBuf,

        #[clap(flatten)]
        args: CliArgs,
    },
}

/// Commands for managing services
#[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
pub enum ServiceCommand {
    /// Generates a new Service JSON.
    Init {
        /// The name of the service (required)
        #[clap(long)]
        name: String,

        /// The ID of the service (optional, autogenerated uuid v7 if not supplied)
        #[clap(long)]
        id: Option<ServiceID>,
    },
    /// Component management commands
    Component {
        #[clap(subcommand)]
        command: ComponentCommand,
    },
}

/// Commands for managing components
#[derive(Debug, Subcommand, Clone, Serialize, Deserialize)]
pub enum ComponentCommand {
    /// Add a component to a service
    Add {
        /// The ID of the component (optional, autogenerated if not supplied)
        #[clap(long)]
        id: Option<ComponentID>,

        /// Path to the WASI component file
        #[clap(long)]
        component: PathBuf,
    },
    /// Manage permissions of a component
    Permissions {
        /// The ID of the component to edit
        #[clap(long)]
        id: ComponentID,

        /// HTTP hosts allowed for access:
        /// Use --http-hosts '' to disallow all hosts
        /// Use --http-hosts '*' to allow all hosts
        /// Use --http-hosts 'host1,host2,...' to allow specific hosts
        /// Omit to leave HTTP permissions unchanged
        #[clap(long, value_delimiter = ',')]
        http_hosts: Option<Vec<String>>,

        /// Enable file system access
        #[clap(long)]
        file_system: Option<bool>,
    },
}

fn parse_service_input(s: &str) -> Result<Service, String> {
    if let Some(path) = s.strip_prefix('@').map(PathBuf::from) {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(&json).map_err(|e| e.to_string())?)
    } else {
        Ok(serde_json::from_str(s).map_err(|e| e.to_string())?)
    }
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

#[derive(Debug, Parser, Clone, Serialize, Deserialize, ValueEnum)]
pub enum CliSubmitKind {
    EthServiceHandler,
    None,
}

impl std::fmt::Display for CliSubmitKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EthServiceHandler => write!(f, "eth-service-handler"),
            Self::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for CliSubmitKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eth-service-handler" => Ok(Self::EthServiceHandler),
            "none" => Ok(Self::None),
            _ => Err(format!("unknown submit kind: {}", s)),
        }
    }
}

impl From<CliSubmitKind> for clap::builder::OsStr {
    fn from(submit: CliSubmitKind) -> clap::builder::OsStr {
        submit.to_string().into()
    }
}

impl Command {
    pub fn args(&self) -> CliArgs {
        let args = match self {
            Self::DeployEigenCore { args, .. } => args,
            Self::DeployEigenServiceManager { args, .. } => args,
            Self::DeployService { args, .. } => args,
            Self::DeployServiceRaw { args, .. } => args,
            Self::UploadComponent { args, .. } => args,
            Self::Exec { args, .. } => args,
            Self::Service { args, .. } => args,
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

    /// Save the deployment (default is true)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_deployment: Option<bool>,

    /// Do not display the results
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_results: Option<bool>,
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
