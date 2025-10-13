use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use utils::config::ConfigExt;
use utils::test_utils::middleware::MiddlewareType;

use crate::e2e::{AnyService, CosmosService, CrossChainService, EvmService, TestMatrix};

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestConfig {
    pub registry: Option<bool>,
    pub mode: TestMode,
    pub middleware_concurrency: bool,
    pub wavs_concurrency: bool,
    pub middleware_type: MiddlewareType,
    pub jaeger: Option<String>,
    pub prometheus: Option<String>,
    _log_levels: Vec<String>,
    _data_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MiddlewareType {
    #[default]
    Eigenlayer,
    Poa,
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
            registry: None,
            _log_levels: EnvFilter::from_default_env()
                .to_string()
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            wavs_concurrency: true,
            middleware_concurrency: false,
            middleware_type: MiddlewareType::default(),
            jaeger: None,
            prometheus: None,
            _data_dir: tempfile::tempdir().unwrap().keep(),
            mode: TestMode::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum TestMode {
    #[default]
    All,
    AllEth,
    AllCosmos,
    Isolated(Vec<AnyService>),
}

impl From<TestMode> for TestMatrix {
    fn from(mode: TestMode) -> Self {
        match mode {
            TestMode::All => {
                // All tests across all chains
                let mut matrix = TestMatrix::default();

                // Add all EVM services
                for service in EvmService::all_values() {
                    matrix.evm.insert(*service);
                }

                // Add all Cosmos services
                for service in CosmosService::all_values() {
                    matrix.cosmos.insert(*service);
                }

                // Add all cross-chain services
                for service in CrossChainService::all_values() {
                    matrix.cross_chain.insert(*service);
                }

                matrix
            }
            TestMode::AllEth => {
                // All EVM tests only
                let mut matrix = TestMatrix::default();

                // Add all EVM services
                for service in EvmService::all_values() {
                    matrix.evm.insert(*service);
                }

                matrix
            }
            TestMode::AllCosmos => {
                // All Cosmos tests only
                let mut matrix = TestMatrix::default();

                // Add all Cosmos services
                for service in CosmosService::all_values() {
                    matrix.cosmos.insert(*service);
                }

                matrix
            }
            TestMode::Isolated(services) => {
                // Only specific services
                let mut matrix = TestMatrix::default();

                for service in services {
                    match service {
                        AnyService::Evm(evm_service) => {
                            matrix.evm.insert(evm_service);
                        }
                        AnyService::Cosmos(cosmos_service) => {
                            matrix.cosmos.insert(cosmos_service);
                        }
                        AnyService::CrossChain(cross_chain_service) => {
                            matrix.cross_chain.insert(cross_chain_service);
                        }
                    }
                }

                matrix
            }
        }
    }
}
