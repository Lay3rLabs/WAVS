use std::path::PathBuf;

use clap::{arg, Parser};
use serde::{Deserialize, Serialize};
use utils::{
    config::{CliEnvExt, ConfigBuilder},
    layer_contract_client::LayerAddresses,
    serde::deserialize_vec_string,
};

use crate::config::Config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    DeployCore {
        #[clap(long, default_value_t = true)]
        register_operator: bool,

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

        #[clap(flatten)]
        args: CliArgs,
    },

    AddTask {
        #[clap(long)]
        service_id: String,

        #[clap(long)]
        workflow_id: Option<String>,

        /// The payload data, hex-encoded
        #[clap(long)]
        input: String,

        #[clap(flatten)]
        args: CliArgs,
    },
}

impl Command {
    pub fn args(&self) -> CliArgs {
        let args = match self {
            Self::DeployCore { args, .. } => args,
            Self::DeployService { args, .. } => args,
            Self::AddTask { args, .. } => args,
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

    /// The chain to use for the application
    /// will load from the config file
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,

    /// mnemonic (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosmos_mnemonic: Option<String>,

    /// mnemonic (usually leave this as None and override in env)
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eth_mnemonic: Option<String>,
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

#[derive(Parser, Debug, Clone)]
pub struct EnvServiceAddresses {
    #[arg(long, env = "CLI_EIGEN_SERVICE_PROXY_ADMIN")]
    pub service_proxy_admin: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_SERVICE_MANAGER")]
    pub service_manager: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_SERVICE_STAKE_REGISTRY")]
    pub service_stake_registry: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_SERVICE_TOKEN")]
    pub service_token: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_SERVICE_TRIGGER")]
    pub service_trigger: Option<alloy::primitives::Address>,
}

impl From<EnvServiceAddresses> for LayerAddresses {
    fn from(opt: EnvServiceAddresses) -> Self {
        Self {
            proxy_admin: opt
                .service_proxy_admin
                .expect("set --service-proxy-admin or CLI_EIGEN_SERVICE_PROXY_ADMIN"),
            service_manager: opt
                .service_manager
                .expect("set --service-manager or CLI_EIGEN_SERVICE_MANAGER"),
            trigger: opt
                .service_trigger
                .expect("set --service-trigger or CLI_EIGEN_SERVICE_TRIGGER"),
            stake_registry: opt
                .service_stake_registry
                .expect("set --service-stake-registry or CLI_EIGEN_SERVICE_STAKE_REGISTRY"),
            token: opt
                .service_token
                .expect("set --service-token or CLI_EIGEN_SERVICE_TOKEN"),
        }
    }
}
