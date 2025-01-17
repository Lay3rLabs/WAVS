use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use utils::config::{ConfigBuilder, ConfigExt};

use crate::{args::TestArgs, e2e::matrix::TestMatrix};

/// The fully parsed and validated config struct we use in the application
/// this is built up from the ConfigBuilder which can load from multiple sources (in order of preference):
///
/// 1. cli args
/// 2. environment variables
/// 3. config file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestConfig {
    pub matrix: TestMatrix,
    isolated: Option<String>,
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
            matrix: TestMatrix::default(),
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

impl TestConfig {
    pub fn new(args: TestArgs) -> Self {
        let mut config: Self = ConfigBuilder::new(args).build().unwrap();

        if let Some(isolated) = &config.isolated {
            config.matrix.overwrite_isolated(isolated);
        }

        config
    }
}
