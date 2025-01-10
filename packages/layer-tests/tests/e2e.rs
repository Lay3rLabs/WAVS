#![allow(warnings)]
// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

mod e2e {
    // TODO
    mod cosmos;
    mod cross_chain;
    mod eth;
    mod http;
    mod payload;

    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };

    use alloy::node_bindings::{Anvil, AnvilInstance};
    use eth::EthTestApp;
    use http::HttpClient;
    use layer_climb::prelude::*;
    use localic_std::transactions::ChainRequestBuilder;
    use serde::{Deserialize, Serialize};
    use tracing_subscriber::EnvFilter;
    use utils::{avs_client::ServiceManagerClient, config::ConfigBuilder};
    use wavs::{
        apis::{
            dispatcher::{ComponentWorld, Submit},
            trigger::Trigger,
            ServiceID,
        },
        args::CliArgs,
    };
    use wavs::{config::Config, dispatcher::CoreDispatcher, AppContext, Digest};

    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn wavs_path() -> PathBuf {
        workspace_path().join("packages").join("wavs")
    }

    fn aggregator_path() -> PathBuf {
        workspace_path().join("packages").join("aggregator")
    }

    #[test]
    fn e2e_tests() {
        if dotenvy::dotenv().is_err() {
            println!("Failed to load .env file");
        }

        let ctx = AppContext::new();

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                tracing::info!("Running Ethereum e2e tests");
                // should match the wavs.toml
                let anvil = Some(Anvil::new().port(8545u16).chain_id(31337).spawn());
                let anvil2 = Some(Anvil::new().port(8645u16).chain_id(31338).spawn());
            } else {
                let anvil: Option<AnvilInstance> = None;
                let anvil2: Option<AnvilInstance> = None;
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_cosmos")] {
                tracing::info!("Running Cosmos e2e tests");
                cosmos::start_chain(ctx.clone());
            } else {
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_crosschain")] {
                tracing::info!("Running Crosschain e2e tests");
            }
        }

        let mut config: wavs::config::Config = ConfigBuilder::new(CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(wavs_path()),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            port: None,
            log_level: Vec::new(),
            host: None,
            cors_allowed_origins: Vec::new(),
            chain: None,
            cosmos_chain: None,
            wasm_lru_size: None,
            wasm_threads: None,
            submission_mnemonic: None,
            cosmos_submission_mnemonic: None,
            max_wasm_fuel: None,
        })
        .build()
        .unwrap();

        let aggregator_config: aggregator::config::Config = {
            let mut cli_args = aggregator::test_utils::app::TestApp::zeroed_cli_args();
            cli_args.home = Some(aggregator_path());
            cli_args.dotenv = None;
            cli_args.chain = Some("local".to_string());
            ConfigBuilder::new(cli_args).build().unwrap()
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_cosmos")] {
                config.cosmos_chain = Some("layer-local".to_string());
            } else {
                config.cosmos_chain = None;
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                config.eth_chains = vec!["local".to_string(), "local2".to_string()];
            } else {
                config.eth_chains = Vec::new();
            }
        }

        let dispatcher = Arc::new(CoreDispatcher::new_core(&config).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = config.clone();
            move || {
                wavs::run_server(ctx, config, dispatcher);
            }
        });

        let aggregator_handle = std::thread::spawn({
            let config = aggregator_config.clone();
            let ctx = ctx.clone();
            move || {
                aggregator::run_server(ctx, config);
            }
        });

        let test_handle = std::thread::spawn({
            move || {
                ctx.rt.clone().block_on({
                    async move {
                        let http_client = HttpClient::new(&config);

                        // give the server a bit of time to start
                        tokio::time::timeout(Duration::from_secs(2), async {
                            loop {
                                match http_client.get_config().await {
                                    Ok(_) => break,
                                    Err(_) => {
                                        tracing::info!("Waiting for server to start...");
                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                    }
                                }
                            }
                        })
                        .await
                        .unwrap();

                        let digests = Digests::new(http_client.clone());
                        let service_ids = ServiceIds::new();

                        match (config.cosmos_chain.is_some(), !config.eth_chains.is_empty()) {
                            (true, false) => {
                                cosmos::run_tests_cosmos(http_client, config, digests, service_ids)
                                    .await
                            }
                            (false, true) => {
                                eth::run_tests_ethereum(
                                    config.eth_chains[0].clone(),
                                    config.eth_chains[1].clone(),
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil.unwrap(),
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil2.unwrap(),
                                    http_client,
                                    config,
                                    digests,
                                    service_ids,
                                )
                                .await;
                            }
                            (true, true) => {
                                cross_chain::run_tests_crosschain(
                                    http_client,
                                    config,
                                    digests,
                                    service_ids,
                                )
                                .await
                            }
                            (false, false) => panic!(
                                "No chain selected at all for e2e tests (see e2e_tests_* features)"
                            ),
                        }
                        ctx.kill();
                    }
                });
            }
        });

        test_handle.join().unwrap();
        wavs_handle.join().unwrap();
        aggregator_handle.join().unwrap();
    }

    pub struct ServiceIds {}

    impl ServiceIds {
        pub fn new() -> Self {
            Self {}
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_square")]
        pub fn eth_trigger_square(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-square").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_1")]
        pub fn eth_trigger_echo_1(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-1").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_2")]
        pub fn eth_trigger_echo_2(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-2").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_aggregate")]
        pub fn eth_trigger_echo_aggregate(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-aggregate").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_cosmos_query")]
        pub fn eth_cosmos_query(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-cosmos-query").unwrap())
        }

        #[cfg(feature = "e2e_tests_cosmos_permissions")]
        pub fn cosmos_permissions(&self) -> Option<ServiceID> {
            Some(ServiceID::new("cosmos-permissions").unwrap())
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_square"))]
        pub fn eth_trigger_square(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_1"))]
        pub fn eth_trigger_echo_1(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_2"))]
        pub fn eth_trigger_echo_2(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_aggregate"))]
        pub fn eth_trigger_echo_aggregate(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_cosmos_query"))]
        pub fn eth_cosmos_query(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_cosmos_permissions"))]
        pub fn cosmos_permissions(&self) -> Option<ServiceID> {
            None
        }
    }

    pub struct Digests {
        http_client: HttpClient,
    }

    impl Digests {
        pub fn new(http_client: HttpClient) -> Self {
            Self { http_client }
        }

        #[cfg(feature = "e2e_tests_cosmos_permissions")]
        pub async fn permissions_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_PERMISSIONS_WASM_DIGEST", "permissions")
                .await
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_square")]
        pub async fn eth_trigger_square_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_TRIGGER_SQUARE_WASM_DIGEST", "square")
                .await
        }

        #[cfg(any(
            feature = "e2e_tests_ethereum_trigger_echo_1",
            feature = "e2e_tests_ethereum_trigger_echo_2"
        ))]
        pub async fn eth_trigger_echo_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_TRIGGER_ECHO_WASM_DIGEST", "echo_data")
                .await
        }

        #[cfg(feature = "e2e_tests_ethereum_cosmos_query")]
        pub async fn eth_cosmos_query(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_COSMOS_QUERY_WASM_DIGEST", "cosmos_query")
                .await
        }

        #[cfg(not(feature = "e2e_tests_cosmos_permissions"))]
        pub async fn permissions_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_square"))]
        pub async fn eth_trigger_square_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(any(
            feature = "e2e_tests_ethereum_trigger_echo_1",
            feature = "e2e_tests_ethereum_trigger_echo_2"
        )))]
        pub async fn eth_trigger_echo_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_cosmos_query"))]
        pub async fn eth_cosmos_query(&self) -> Option<Digest> {
            None
        }

        async fn get_digest(&self, env_var_key: &str, wasm_filename: &str) -> Option<Digest> {
            let digest = std::env::var(env_var_key);

            let digest: Digest = match digest {
                Ok(digest) => digest.parse().unwrap(),
                Err(_) => {
                    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("examples")
                        .join("build")
                        .join("components")
                        .join(format!("{}.wasm", wasm_filename));

                    tracing::info!("Uploading wasm: {}", wasm_path.display());

                    let wasm_bytes = tokio::fs::read(wasm_path).await.unwrap();
                    self.http_client
                        .upload_wasm(wasm_bytes.to_vec())
                        .await
                        .unwrap()
                }
            };

            Some(digest)
        }
    }
}
