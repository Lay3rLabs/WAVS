use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use utils::config::ConfigExt;

use crate::e2e::matrix::{AnyService, CosmosService, CrossChainService, EvmService, TestMatrix};

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestConfig {
    pub matrix: TestMatrixConfig,
    pub registry: Option<bool>,
    pub registry_domain: Option<String>,
    pub isolated: Option<String>,
    pub all: Option<bool>,
    pub jaeger: Option<String>,
    pub prometheus: Option<String>,
    _log_levels: Vec<String>,
    _data_dir: PathBuf,
}

impl ConfigExt for TestConfig {
    const FILENAME: &'static str = "layer-tests.toml";

    fn with_data_dir(&mut self, f: fn(&mut PathBuf)) {
        f(&mut self._data_dir);
    }

    fn log_levels(&self) -> impl Iterator<Item = &str> {
        self._log_levels.iter().map(|s| s.as_str())
    }
}

/// Default values for the config struct
/// these are only used to fill in holes after all the parsing and loading is done
impl Default for TestConfig {
    fn default() -> Self {
        Self {
            matrix: TestMatrixConfig::default(),
            registry: None,
            registry_domain: None,
            _log_levels: EnvFilter::from_default_env()
                .to_string()
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            jaeger: None,
            prometheus: None,
            _data_dir: tempfile::tempdir().unwrap().into_path(),
            isolated: None,
            all: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TestMatrixConfig {
    pub evm: TestMatrixEvmConfig,
    pub cosmos: TestMatrixCosmosConfig,
    pub crosschain: TestMatrixCrossChainConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixEvmConfig {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub echo_data_secondary_chain: bool,
    pub echo_data_aggregator: bool,
    pub permissions: bool,
    pub square: bool,
    pub multi_workflow: bool,
    pub multi_trigger: bool,
    pub block_interval: bool,
    pub cron_interval: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCosmosConfig {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub permissions: bool,
    pub square: bool,
    pub block_interval: bool,
    pub cron_interval: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCrossChainConfig {
    pub cosmos_to_evm_echo_data: bool,
}

impl TestMatrixConfig {
    pub fn into_validated(self, all: Option<bool>, isolated: Option<&str>) -> TestMatrix {
        let mut matrix = TestMatrix::default();

        match (all, isolated) {
            (Some(true), Some(_)) => {
                panic!("Cannot specify both --all and --isolated");
            }
            (Some(true), _) => {
                for service in EvmService::all_values() {
                    matrix.evm.insert(*service);
                }

                for service in CosmosService::all_values() {
                    matrix.cosmos.insert(*service);
                }

                for service in CrossChainService::all_values() {
                    matrix.cross_chain.insert(*service);
                }
            }
            (_, Some(isolated)) => {
                match AnyService::from(isolated) {
                    AnyService::EVM(service) => {
                        matrix.evm.insert(service);
                    }
                    AnyService::Cosmos(service) => {
                        matrix.cosmos.insert(service);
                    }
                    AnyService::CrossChain(service) => {
                        matrix.cross_chain.insert(service);
                    }
                }

                return matrix;
            }
            _ => {}
        }

        if self.evm.chain_trigger_lookup {
            matrix.evm.insert(EvmService::ChainTriggerLookup);
        }

        if self.evm.cosmos_query {
            matrix.evm.insert(EvmService::CosmosQuery);
        }

        if self.evm.echo_data {
            matrix.evm.insert(EvmService::EchoData);
        }

        if self.evm.echo_data_secondary_chain {
            matrix.evm.insert(EvmService::EchoDataSecondaryChain);
        }

        if self.evm.echo_data_aggregator {
            matrix.evm.insert(EvmService::EchoDataAggregator);
        }

        if self.evm.permissions {
            matrix.evm.insert(EvmService::Permissions);
        }

        if self.evm.square {
            matrix.evm.insert(EvmService::Square);
        }

        if self.evm.multi_workflow {
            matrix.evm.insert(EvmService::MultiWorkflow);
        }

        if self.evm.multi_trigger {
            matrix.evm.insert(EvmService::MultiTrigger);
        }

        if self.evm.block_interval {
            matrix.evm.insert(EvmService::BlockInterval);
        }

        if self.evm.cron_interval {
            matrix.evm.insert(EvmService::CronInterval);
        }

        if self.cosmos.chain_trigger_lookup {
            matrix.cosmos.insert(CosmosService::ChainTriggerLookup);
        }

        if self.cosmos.cosmos_query {
            matrix.cosmos.insert(CosmosService::CosmosQuery);
        }

        if self.cosmos.echo_data {
            matrix.cosmos.insert(CosmosService::EchoData);
        }

        if self.cosmos.permissions {
            matrix.cosmos.insert(CosmosService::Permissions);
        }

        if self.cosmos.square {
            matrix.cosmos.insert(CosmosService::Square);
        }

        if self.cosmos.block_interval {
            matrix.cosmos.insert(CosmosService::BlockInterval);
        }

        if self.cosmos.cron_interval {
            matrix.cosmos.insert(CosmosService::CronInterval);
        }

        if self.crosschain.cosmos_to_evm_echo_data {
            matrix
                .cross_chain
                .insert(CrossChainService::CosmosToEvmEchoData);
        }

        matrix
    }
}

impl From<&str> for AnyService {
    fn from(src: &str) -> Self {
        match src {
            "evm-chain-trigger-lookup" => AnyService::EVM(EvmService::ChainTriggerLookup),
            "evm-cosmos-query" => AnyService::EVM(EvmService::CosmosQuery),
            "evm-echo-data" => AnyService::EVM(EvmService::EchoData),
            "evm-echo-data-secondary-chain" => AnyService::EVM(EvmService::EchoDataSecondaryChain),
            "evm-echo-data-aggregator" => AnyService::EVM(EvmService::EchoDataAggregator),
            "evm-permissions" => AnyService::EVM(EvmService::Permissions),
            "evm-square" => AnyService::EVM(EvmService::Square),
            "evm-multi-workflow" => AnyService::EVM(EvmService::MultiWorkflow),
            "evm-multi-trigger" => AnyService::EVM(EvmService::MultiTrigger),
            "evm-block-interval" => AnyService::EVM(EvmService::BlockInterval),
            "evm-cron-interval" => AnyService::EVM(EvmService::CronInterval),
            "cosmos-chain-trigger-lookup" => AnyService::Cosmos(CosmosService::ChainTriggerLookup),
            "cosmos-cosmos-query" => AnyService::Cosmos(CosmosService::CosmosQuery),
            "cosmos-echo-data" => AnyService::Cosmos(CosmosService::EchoData),
            "cosmos-permissions" => AnyService::Cosmos(CosmosService::Permissions),
            "cosmos-square" => AnyService::Cosmos(CosmosService::Square),
            "cosmos-block-interval" => AnyService::Cosmos(CosmosService::BlockInterval),
            "cosmos-cron-interval" => AnyService::Cosmos(CosmosService::CronInterval),
            "crosschain-cosmos-to-evm-echo-data" => {
                AnyService::CrossChain(CrossChainService::CosmosToEvmEchoData)
            }
            _ => panic!("Unknown service: {}", src),
        }
    }
}
