use std::{path::PathBuf, sync::Arc};

use wasmatic::{
    args::CliArgs,
    config::{Config, ConfigBuilder},
};

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn default_cli_args() -> CliArgs {
        // get the path relative from this source file, regardless of where we run the test from
        CliArgs {
            home: Some(
                PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(ConfigBuilder::DIRNAME),
            ),
            // this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(
                PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(ConfigBuilder::DIRNAME)
                    .join("non-existing-testdotenv"),
            ),
            port: None,
            log_level: Vec::new(),
            host: None,
            data: None,
            cors_allowed_origins: Vec::new(),
            chain: None,
            chain_config: Default::default(),
        }
    }

    pub async fn new() -> Self {
        Self::new_with_args(Self::default_cli_args()).await
    }

    pub async fn new_with_args(cli_args: CliArgs) -> Self {
        let config = Arc::new(ConfigBuilder::new(cli_args).build().unwrap());

        wasmatic::init_tracing_tests();

        Self { config }
    }
}
