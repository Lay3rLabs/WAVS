use std::{path::PathBuf, sync::Arc};

use crate::{
    args::CliArgs,
    config::{Config, ConfigBuilder},
};

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn cli_args(endpoint: String) -> CliArgs {
        // get the path relative from this source file, regardless of where we run the test from
        CliArgs {
            home: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(ConfigBuilder::DIRNAME),
            ),
            // this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(ConfigBuilder::DIRNAME)
                    .join("non-existant-file"),
            ),
            port: None,
            log_level: Vec::new(),
            host: None,
            // Anvil default memo
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_owned(),
            ),
            cors_allowed_origins: Vec::new(),
            chain: None,
            endpoint: Some(endpoint),
        }
    }

    pub async fn new(endpoint: String) -> Self {
        Self::new_with_args(Self::cli_args(endpoint)).await
    }

    pub async fn new_with_args(cli_args: CliArgs) -> Self {
        let config = Arc::new(ConfigBuilder::new(cli_args).build().unwrap());

        crate::init_tracing_tests();

        Self { config }
    }
}
