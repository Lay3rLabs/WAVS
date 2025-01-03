use alloy::node_bindings::AnvilInstance;
use std::{path::PathBuf, sync::Arc};
use utils::{
    config::{ConfigBuilder, ConfigExt, EthereumChainConfig},
    eth_client::{EthChainConfig, EthClientBuilder, EthClientConfig, EthSigningClient},
};

use crate::{args::CliArgs, config::Config};

const ANVIL_DEFAULT_MNEMONIC: &str = "test test test test test test test test test test test junk";

#[derive(Clone)]
pub struct TestApp {
    pub config: Arc<Config>,
}

impl TestApp {
    pub fn zeroed_cli_args() -> CliArgs {
        // get the path relative from this source file, regardless of where we run the test from
        CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            // while this technically isn't "zeroed", this purposefully points at a non-existing file
            // so that we don't load a real .env in tests
            dotenv: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests")
                    .join(Config::DIRNAME)
                    .join("non-existant-file"),
            ),
            port: None,
            log_level: Vec::new(),
            host: None,
            mnemonic: None,
            cors_allowed_origins: Vec::new(),
            chain: None,
            hd_index: None,
            tasks_quorum: Some(1),
        }
    }

    pub fn new(anvil: Option<&AnvilInstance>) -> Self {
        Self::new_with_args(Self::zeroed_cli_args(), anvil)
    }

    pub fn new_with_args(mut cli_args: CliArgs, anvil: Option<&AnvilInstance>) -> Self {
        if anvil.is_some() {
            cli_args.mnemonic = Some(ANVIL_DEFAULT_MNEMONIC.to_owned());
        }

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

    pub async fn eth_signing_client(&self) -> EthSigningClient {
        let chain = self
            .config
            .chains
            .get_chain(&self.config.chain)
            .unwrap()
            .unwrap()
            .clone();
        let chain: EthereumChainConfig = chain.try_into().unwrap();
        let chain: EthChainConfig = chain.into();
        let client_config: EthClientConfig =
            chain.to_client_config(None, Some(ANVIL_DEFAULT_MNEMONIC.to_owned()));

        EthClientBuilder::new(client_config)
            .build_signing()
            .await
            .unwrap()
    }
}
