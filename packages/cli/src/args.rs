use std::path::PathBuf;

use clap::{arg, Parser};
use serde::{Deserialize, Serialize};
use utils::{
    config::CliEnvExt, eigen_client::CoreAVSAddresses, layer_contract_client::LayerAddresses,
    serde::deserialize_vec_string,
};
use wavs::Digest;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    DeployCore {
        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(flatten)]
        args: CliArgs,
    },

    DeployAll {
        /// If set, will add the service to wavs too
        #[clap(long, default_value_t = false)]
        add_service: bool,

        #[clap(long, default_value_t = true)]
        register_core_operator: bool,

        #[clap(long, default_value_t = true)]
        register_service_operator: bool,

        #[clap(flatten)]
        digests: Digests,

        #[clap(flatten)]
        args: CliArgs,
    },

    DeployService {
        /// If set, will add the service to wavs too
        #[clap(long, default_value_t = false)]
        add_service: bool,

        #[clap(long, default_value_t = true)]
        register_operator: bool,

        #[clap(flatten)]
        core_contracts: EnvCoreAVSAddresses,

        #[clap(flatten)]
        digests: Digests,

        #[clap(flatten)]
        args: CliArgs,
    },

    AddTask {
        /// If set, will watch the chain for final result
        /// otherwise, will manually submit the result to the contract
        #[clap(long, default_value_t = false)]
        watch_wavs: bool,

        /// The contract address for the trigger
        #[clap(long, env = "CLI_EIGEN_SERVICE_TRIGGER")]
        trigger_addr: alloy::primitives::Address,

        /// The contract address for the service manager
        #[clap(long, env = "CLI_EIGEN_SERVICE_MANAGER")]
        service_manager_addr: alloy::primitives::Address,

        #[clap(long)]
        service_id: String,

        #[clap(long)]
        workflow_id: Option<String>,

        /// The name of the task
        /// if not set, will be a random string
        #[clap(long)]
        name: Option<String>,

        #[clap(flatten)]
        args: CliArgs,
    },

    /// Kitchen sink subcommand
    KitchenSink {
        /// If set, will add the service to wavs
        /// and wait for the final result to land
        /// otherwise, will manually submit the result to the contract
        #[clap(long, default_value_t = false)]
        add_service: bool,

        #[clap(long, default_value_t = true)]
        register_core_operator: bool,

        #[clap(long, default_value_t = true)]
        register_service_operator: bool,

        #[clap(flatten)]
        digests: Digests,

        /// The name of the task
        /// if not set, will be a random string
        #[clap(long)]
        name: Option<String>,

        #[clap(flatten)]
        args: CliArgs,
    },
}

/// This struct is used for both args and environment variables
/// the basic idea is that every env var can be overriden by a cli arg
/// and these override the config file
/// env vars follow the pattern of WAVS_CLI_{UPPERCASE_ARG_NAME}
#[derive(Debug, Parser, Serialize, Deserialize, Default)]
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
pub struct EnvCoreAVSAddresses {
    #[arg(long, env = "CLI_EIGEN_CORE_PROXY_ADMIN")]
    pub core_proxy_admin: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_DELEGATION_MANAGER")]
    pub core_delegation_manager: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_STRATEGY_MANAGER")]
    pub core_strategy_manager: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_POD_MANAGER")]
    pub core_eigen_pod_manager: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_POD_BEACON")]
    pub core_eigen_pod_beacon: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_PAUSER_REGISTRY")]
    pub core_pauser_registry: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_STRATEGY_FACTORY")]
    pub core_strategy_factory: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_STRATEGY_BEACON")]
    pub core_strategy_beacon: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_AVS_DIRECTORY")]
    pub core_avs_directory: Option<alloy::primitives::Address>,
    #[arg(long, env = "CLI_EIGEN_CORE_REWARDS_COORDINATOR")]
    pub core_rewards_coordinator: Option<alloy::primitives::Address>,
}

impl From<EnvCoreAVSAddresses> for CoreAVSAddresses {
    fn from(opt: EnvCoreAVSAddresses) -> Self {
        Self {
            proxy_admin: opt
                .core_proxy_admin
                .expect("set --core-proxy-admin or CLI_EIGEN_CORE_PROXY_ADMIN"),
            delegation_manager: opt
                .core_delegation_manager
                .expect("set --core-delegation-manager or CLI_EIGEN_CORE_DELEGATION_MANAGER"),
            strategy_manager: opt
                .core_strategy_manager
                .expect("set --core-strategy-manager or CLI_EIGEN_CORE_STRATEGY_MANAGER"),
            eigen_pod_manager: opt
                .core_eigen_pod_manager
                .expect("set --core-eigen-pod-manager or CLI_EIGEN_CORE_POD_MANAGER"),
            eigen_pod_beacon: opt
                .core_eigen_pod_beacon
                .expect("set --core-eigen-pod-beacon or CLI_EIGEN_CORE_POD_BEACON"),
            pauser_registry: opt
                .core_pauser_registry
                .expect("set --core-pauser-registry or CLI_EIGEN_CORE_PAUSER_REGISTRY"),
            strategy_factory: opt
                .core_strategy_factory
                .expect("set --core-strategy-factory or CLI_EIGEN_CORE_STRATEGY_FACTORY"),
            strategy_beacon: opt
                .core_strategy_beacon
                .expect("set --core-strategy-beacon or CLI_EIGEN_CORE_STRATEGY_BEACON"),
            avs_directory: opt
                .core_avs_directory
                .expect("set --core-avs-directory or CLI_EIGEN_CORE_AVS_DIRECTORY"),
            rewards_coordinator: opt
                .core_rewards_coordinator
                .expect("set --core-rewards-coordinator or CLI_EIGEN_CORE_REWARDS_COORDINATOR"),
        }
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

#[derive(Parser, Debug, Clone)]
pub struct Digests {
    #[arg(long, env = "CLI_DIGEST_HELLO_WORLD")]
    pub digest_hello_world: Option<Digest>,
}
