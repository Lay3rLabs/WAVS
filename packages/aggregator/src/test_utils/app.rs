use alloy::node_bindings::AnvilInstance;
use std::{path::PathBuf, sync::Arc};
use utils::config::{ConfigBuilder, ConfigExt, EthereumChainConfig};

use crate::{args::CliArgs, config::Config};

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn default_cli_args() -> CliArgs {
        // get the path relative from this source file, regardless of where we run the test from
        CliArgs {
            home: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(Config::DIRNAME),
            ),
            // this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(Config::DIRNAME)
                    .join("non-existant-file"),
            ),
            data: None,
            port: None,
            log_level: Vec::new(),
            host: None,
            // Anvil default memo
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_owned(),
            ),
            cors_allowed_origins: Vec::new(),
            chain: None,
            hd_index: None,
            tasks_quorum: Some(1),
        }
    }

    pub fn new(anvil: Option<&AnvilInstance>) -> Self {
        Self::new_with_args(Self::default_cli_args(), anvil)
    }

    pub fn new_with_args(cli_args: CliArgs, anvil: Option<&AnvilInstance>) -> Self {
        let mut config: Config = ConfigBuilder::new(cli_args).build().unwrap();

        if let Some(anvil) = anvil {
            let chain = config
                .chains
                .get_chain(&config.chain)
                .unwrap()
                .unwrap()
                .clone();
            let mut chain: EthereumChainConfig = chain.try_into().unwrap();
            chain.ws_endpoint = anvil.ws_endpoint().to_string();
            chain.http_endpoint = anvil.endpoint().to_string();
            config.chains.eth.insert(config.chain.clone(), chain);
        }

        crate::init_tracing_tests();

        Self {
            config: Arc::new(config),
        }
    }
}
