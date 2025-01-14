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
    use cosmos::{CosmosTestApp, IcTestHandle};
    use eth::EthTestApp;
    use http::HttpClient;
    use layer_climb::prelude::*;
    use serde::{Deserialize, Serialize};
    use tracing_subscriber::EnvFilter;
    use utils::{
        avs_client::ServiceManagerClient,
        config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EthereumChainConfig},
    };
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

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        let ctx = AppContext::new();

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                let mut eth_chains = vec![
                    eth::start_chain(ctx.clone(), 0),
                    eth::start_chain(ctx.clone(), 1),
                ];
            } else {
                let mut eth_chains:Vec<(String, EthereumChainConfig, Option<AnvilInstance>)> = Vec::new();
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_cosmos")] {
                let mut cosmos_chains = vec![
                    cosmos::start_chain(ctx.clone(), 0),
                ];
            } else {
                let mut cosmos_chains:Vec<(String, CosmosChainConfig, Option<IcTestHandle>)> = Vec::new();
            }
        }

        let mut config: wavs::config::Config = ConfigBuilder::new(CliArgs {
            data: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
            home: Some(wavs_path()),
            // deliberately point to a non-existing file
            dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .unwrap();

        config.eth_chains = eth_chains.iter().map(|(name, _, _)| name.clone()).collect();
        config.cosmos_chain = cosmos_chains.first().map(|(name, _, _)| name.clone());
        config.chains = ChainConfigs{
            cosmos: cosmos_chains.iter().map(|(name, chain, _)| (name.clone(), chain.clone())).collect(), 
            eth: eth_chains.iter().map(|(name, chain, _)| (name.clone(), chain.clone())).collect(), 
        };

        let aggregator_config: aggregator::config::Config = {
            let mut cli_args = aggregator::test_utils::app::TestApp::zeroed_cli_args();
            cli_args.home = Some(aggregator_path());
            cli_args.dotenv = None;
            cli_args.chain = Some("local".to_string());
            ConfigBuilder::new(cli_args).build().unwrap()
        };

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

                        let digests = Digests::new(&http_client).await;
                        let service_ids = ServiceIds::new();

                        let mut eth_apps = Vec::new();
                        for (name, chain_config, handle) in eth_chains.drain(..) {
                            eth_apps.push(EthTestApp::new(name, chain_config, handle).await);
                        }

                        let mut cosmos_apps = Vec::new();
                        for (name, chain_config, handle) in cosmos_chains.drain(..) {
                            cosmos_apps.push(CosmosTestApp::new(name, chain_config, handle).await);
                        }

                        if !eth_apps.is_empty() {
                            eth::run_tests(eth_apps.clone(), http_client.clone(), digests.clone(), service_ids.clone())
                                .await
                        }
                        if !cosmos_apps.is_empty() {
                            cosmos::run_tests(cosmos_apps.clone(), http_client.clone(), digests.clone(), service_ids.clone())
                                .await
                        }

                        if !eth_apps.is_empty() && !cosmos_apps.is_empty() && cfg!(feature = "e2e_tests_crosschain") {
                            cross_chain::run_tests_crosschain(
                                eth_apps.clone(),
                                cosmos_apps.clone(),
                                http_client,
                                digests,
                                service_ids,
                            )
                            .await
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

    #[derive(Clone)]
    pub struct ServiceIds {
        pub eth_square: Option<ServiceID>,
        pub eth_echo_1: Option<ServiceID>,
        pub eth_echo_2: Option<ServiceID>,
        pub eth_echo_aggregate: Option<ServiceID>,
        pub eth_cosmos_query: Option<ServiceID>,
        pub eth_permissions: Option<ServiceID>,
        pub cosmos_permissions: Option<ServiceID>,
    }

    impl ServiceIds {
        pub fn new() -> Self {
            Self {
                eth_square: if cfg!(feature = "e2e_tests_ethereum_trigger_square") {
                        Some(ServiceID::new("eth-trigger-square").unwrap())
                } else {
                    None
                },
                eth_echo_1: if cfg!(feature = "e2e_tests_ethereum_trigger_echo_1") {
                    Some(ServiceID::new("eth-trigger-echo-1").unwrap())
                } else {
                    None
                },
                eth_echo_2: if cfg!(feature = "e2e_tests_ethereum_trigger_echo_2") {
                        Some(ServiceID::new("eth-trigger-echo-2").unwrap())
                } else {
                    None
                },
                eth_echo_aggregate: if cfg!(feature = "e2e_tests_ethereum_trigger_echo_aggregate") { 
                        Some(ServiceID::new("eth-trigger-echo-aggregate").unwrap())
                } else {
                    None
                },
                eth_cosmos_query: if cfg!(feature = "e2e_tests_ethereum_cosmos_query") {
                        Some(ServiceID::new("eth-cosmos-query").unwrap())
                } else {
                    None
                },
                eth_permissions: if cfg!(feature = "e2e_tests_ethereum_permissions") {
                        Some(ServiceID::new("eth-permissions").unwrap())
                } else {
                    None
                },
                cosmos_permissions: if cfg!(feature = "e2e_tests_cosmos_permissions") {
                        Some(ServiceID::new("cosmos-permissions").unwrap())
                } else {
                    None
                },
            }
        }
    }

    #[derive(Clone)]
    pub struct Digests {
        permissions: Option<Digest>,
        square: Option<Digest>,
        echo_eth_event: Option<Digest>,
        echo_cosmos_event: Option<Digest>,
        echo_raw: Option<Digest>,
        cosmos_query: Option<Digest>,
        cosmos_trigger_lookup: Option<Digest>,
    }

    impl Digests {
        pub async fn new(http_client: &HttpClient) -> Self {
            async fn get_digest(http_client: &HttpClient, env_var_key: &str, wasm_filename: &str) -> Option<Digest> {
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

                        http_client
                            .upload_wasm(wasm_bytes.to_vec())
                            .await
                            .unwrap()
                    }
                };

                Some(digest)
            }
            Self { 
                permissions: if cfg!(feature = "e2e_tests_cosmos_permissions") {
                    Some(get_digest(http_client, "WAVS_E2E_PERMISSIONS_WASM_DIGEST", "permissions").await.unwrap())
                } else {
                    None
                },
                square: if cfg!(feature = "e2e_tests_ethereum_trigger_square") {
                    Some(get_digest(http_client, "WAVS_E2E_SQUARE_WASM_DIGEST", "square").await.unwrap())
                } else {
                    None
                },
                echo_eth_event: if cfg!(feature = "e2e_tests_ethereum_trigger_echo_1") || cfg!(feature = "e2e_tests_ethereum_trigger_echo_2") || cfg!(feature = "e2e_tests_ethereum_trigger_echo_aggregate") {
                    Some(get_digest(http_client, "WAVS_E2E_ECHO_ETH_EVENT_WASM_DIGEST", "echo_eth_event").await.unwrap())
                } else {
                    None
                },
                echo_cosmos_event: if cfg!(feature = "e2e_tests_cosmos_trigger_echo") {
                    Some(get_digest(http_client, "WAVS_E2E_ECHO_COSMOS_EVENT_WASM_DIGEST", "echo_cosmos_event").await.unwrap())
                } else {
                    None
                },
                echo_raw: None,
                cosmos_query: if cfg!(feature = "e2e_tests_ethereum_cosmos_query") {
                    Some(get_digest(http_client, "WAVS_E2E_COSMOS_QUERY_WASM_DIGEST", "cosmos_query").await.unwrap())
                } else {
                    None
                },
                cosmos_trigger_lookup: if cfg!(feature = "e2e_tests_ethereum_cosmos_query") {
                    Some(get_digest(http_client, "WAVS_E2E_COSMOS_TRIGGER_LOOKUP_WASM_DIGEST", "cosmos_trigger_lookup").await.unwrap())
                } else {
                    None
                },
            }
        }
    }
}
