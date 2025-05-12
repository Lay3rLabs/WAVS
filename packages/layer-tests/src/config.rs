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
    pub registry: Option<bool>,
    pub registry_domain: Option<String>,
    pub mode: TestMode,
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
            registry: None,
            registry_domain: None,
            _log_levels: EnvFilter::from_default_env()
                .to_string()
                .split(',')
                .map(|s| s.to_string())
                .collect(),
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
        let mut matrix = TestMatrix::default();

        match mode {
            TestMode::All => {
                matrix.evm = EvmService::all_values().iter().cloned().collect();
                matrix.cosmos = CosmosService::all_values().iter().cloned().collect();
                matrix.cross_chain = CrossChainService::all_values().iter().cloned().collect();
            }
            TestMode::AllEth => {
                matrix.evm = EvmService::all_values().iter().cloned().collect();
            }
            TestMode::AllCosmos => {
                matrix.cosmos = CosmosService::all_values().iter().cloned().collect();
            }
            TestMode::Isolated(services) => {
                for service in services {
                    match service {
                        AnyService::Evm(s) => matrix.evm.insert(s),
                        AnyService::Cosmos(s) => matrix.cosmos.insert(s),
                        AnyService::CrossChain(s) => matrix.cross_chain.insert(s),
                    };
                }
            }
        }

        matrix
    }
}
