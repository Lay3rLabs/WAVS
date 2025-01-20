use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use utils::config::ConfigExt;

use crate::e2e::matrix::{AnyService, CosmosService, CrossChainService, EthService, TestMatrix};

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestConfig {
    pub matrix: TestMatrixConfig,
    pub isolated: Option<String>,
    _log_levels: Vec<String>,
    _data_dir: PathBuf,
}

impl ConfigExt for TestConfig {
    const DIRNAME: &'static str = "layer-tests";
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
            _log_levels: EnvFilter::from_default_env()
                .to_string()
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            _data_dir: tempfile::tempdir().unwrap().into_path(),
            isolated: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TestMatrixConfig {
    pub eth: TestMatrixEthConfig,
    pub cosmos: TestMatrixCosmosConfig,
    pub crosschain: TestMatrixCrossChainConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixEthConfig {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub echo_data_secondary_chain: bool,
    pub echo_data_aggregator: bool,
    pub permissions: bool,
    pub square: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCosmosConfig {
    pub chain_trigger_lookup: bool,
    pub cosmos_query: bool,
    pub echo_data: bool,
    pub permissions: bool,
    pub square: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TestMatrixCrossChainConfig {
    pub cosmos_to_eth_echo_data: bool,
}

impl TestMatrixConfig {
    pub fn into_validated(self, isolated: Option<&str>) -> TestMatrix {
        let mut matrix = TestMatrix::default();

        if let Some(isolated) = isolated {
            match AnyService::from(isolated) {
                AnyService::Eth(service) => {
                    matrix.eth.insert(service);
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

        if self.eth.chain_trigger_lookup {
            matrix.eth.insert(EthService::ChainTriggerLookup);
        }

        if self.eth.cosmos_query {
            matrix.eth.insert(EthService::CosmosQuery);
        }

        if self.eth.echo_data {
            matrix.eth.insert(EthService::EchoData);
        }

        if self.eth.echo_data_secondary_chain {
            matrix.eth.insert(EthService::EchoDataSecondaryChain);
        }

        if self.eth.echo_data_aggregator {
            matrix.eth.insert(EthService::EchoDataAggregator);
        }

        if self.eth.permissions {
            matrix.eth.insert(EthService::Permissions);
        }

        if self.eth.square {
            matrix.eth.insert(EthService::Square);
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

        if self.crosschain.cosmos_to_eth_echo_data {
            matrix
                .cross_chain
                .insert(CrossChainService::CosmosToEthEchoData);
        }

        matrix
    }
}

impl From<&str> for AnyService {
    fn from(src: &str) -> Self {
        match src {
            "eth_chain_trigger_lookup" => AnyService::Eth(EthService::ChainTriggerLookup),
            "eth_cosmos_query" => AnyService::Eth(EthService::CosmosQuery),
            "eth_echo_data" => AnyService::Eth(EthService::EchoData),
            "eth_echo_data_secondary_chain" => AnyService::Eth(EthService::EchoDataSecondaryChain),
            "eth_echo_data_aggregator" => AnyService::Eth(EthService::EchoDataAggregator),
            "eth_permissions" => AnyService::Eth(EthService::Permissions),
            "eth_square" => AnyService::Eth(EthService::Square),
            "cosmos_chain_trigger_lookup" => AnyService::Cosmos(CosmosService::ChainTriggerLookup),
            "cosmos_cosmos_query" => AnyService::Cosmos(CosmosService::CosmosQuery),
            "cosmos_echo_data" => AnyService::Cosmos(CosmosService::EchoData),
            "cosmos_permissions" => AnyService::Cosmos(CosmosService::Permissions),
            "cosmos_square" => AnyService::Cosmos(CosmosService::Square),
            "crosschain_cosmos_to_eth_echo_data" => {
                AnyService::CrossChain(CrossChainService::CosmosToEthEchoData)
            }
            _ => panic!("Unknown service: {}", src),
        }
    }
}
