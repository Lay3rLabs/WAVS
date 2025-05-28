use std::{path::PathBuf, sync::Arc};

use utils::config::ConfigBuilder;

use crate::{args::CliArgs, config::Config};

use super::mock_chain_configs::mock_chain_configs;

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
    // need to hold onto the tempdir handles so that they don't get dropped
    _temp_home_dir_handle: Arc<tempfile::TempDir>,
    _temp_data_dir_handle: Arc<tempfile::TempDir>,
}

impl TestApp {
    pub async fn new() -> Self {
        let temp_home_dir_handle = Arc::new(tempfile::tempdir().unwrap());
        let temp_data_dir_handle = Arc::new(tempfile::tempdir().unwrap());

        let mut config: Config = ConfigBuilder::new(zeroed_cli_args(
            temp_home_dir_handle.clone(),
            temp_data_dir_handle.clone(),
        ))
        .build()
        .unwrap();

        config.chains = mock_chain_configs();

        crate::init_tracing_tests();

        Self {
            config: Arc::new(config),
            _temp_home_dir_handle: temp_home_dir_handle,
            _temp_data_dir_handle: temp_data_dir_handle,
        }
    }
}

fn zeroed_cli_args(
    temp_home_dir_handle: Arc<tempfile::TempDir>,
    temp_data_dir_handle: Arc<tempfile::TempDir>,
) -> CliArgs {
    // write wavs.toml empty data into home_dir
    let wavs_toml = temp_home_dir_handle.path().join("wavs.toml");
    std::fs::write(&wavs_toml, "").unwrap();

    CliArgs {
        data: Some(temp_data_dir_handle.path().to_path_buf()),
        home: Some(temp_home_dir_handle.path().to_path_buf()),
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
        max_execution_seconds: None,
        ipfs_gateway: None,
        submission_poll_interval_ms: None,
    }
}
