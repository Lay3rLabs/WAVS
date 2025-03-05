use std::{path::PathBuf, sync::Arc};

use utils::config::ConfigBuilder;

use crate::{args::CliArgs, config::Config, engine::mock::mock_chain_configs};

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn zeroed_cli_args() -> CliArgs {
        CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            // while this technically isn't "zeroed", this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("non-existant-file")),
            port: None,
            log_level: Vec::new(),
            host: None,
            cors_allowed_origins: Vec::new(),
            wasm_lru_size: None,
            wasm_threads: None,
            submission_mnemonic: None,
            cosmos_submission_mnemonic: None,
            max_wasm_fuel: None,
        }
    }

    pub async fn new() -> Self {
        Self::new_with_args(Self::zeroed_cli_args()).await
    }

    pub async fn new_with_args(cli_args: CliArgs) -> Self {
        let mut config: Config = ConfigBuilder::new(cli_args).build().unwrap();

        config.chains = mock_chain_configs();

        crate::init_tracing_tests();

        Self {
            config: Arc::new(config),
        }
    }
}
