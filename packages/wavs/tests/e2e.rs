// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

#[cfg(feature = "e2e_tests")]
mod e2e {
    mod cosmos;
    mod eth;
    mod http;

    use std::{path::PathBuf, sync::Arc, time::Duration};

    use alloy::node_bindings::{Anvil, AnvilInstance};
    use anyhow::bail;
    use cosmos::CosmosTestApp;
    use eth::EthTestApp;
    use http::HttpClient;
    use lavs_apis::{
        events::{task_queue_events::TaskCreatedEvent, traits::TypedEvent},
        tasks as task_queue,
    };
    use layer_climb::prelude::*;
    use serde::{Deserialize, Serialize};
    use wavs::{
        apis::{dispatcher::Submit, ServiceID},
        test_utils::app::TestApp,
    };
    use wavs::{config::Config, dispatcher::CoreDispatcher, AppContext, Digest};

    #[test]
    fn e2e_tests() {
        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                let anvil = Some(Anvil::new().spawn());
            } else {
                let anvil: Option<AnvilInstance> = None;
            }
        }
        let mut config = {
            tokio::runtime::Runtime::new().unwrap().block_on({
                async {
                    let mut cli_args = TestApp::default_cli_args();
                    cli_args.dotenv = None;
                    cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
                    if let Some(anvil) = anvil.as_ref() {
                        cli_args.chain_config.ws_endpoint = Some(anvil.ws_endpoint().to_string());
                        cli_args.chain_config.http_endpoint = Some(anvil.endpoint().to_string());
                    }
                    TestApp::new_with_args(cli_args)
                        .await
                        .config
                        .as_ref()
                        .clone()
                }
            })
        };
        let aggregator_config: aggregator::config::Config = {
            let mut cli_args = aggregator::test_utils::app::TestApp::default_cli_args();
            cli_args.dotenv = None;
            if let Some(anvil) = anvil.as_ref() {
                cli_args.ws_endpoint = Some(anvil.ws_endpoint().to_string());
                cli_args.http_endpoint = Some(anvil.endpoint().to_string());
            }
            aggregator::config::ConfigBuilder::new(cli_args)
                .build()
                .unwrap()
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_cosmos")] {
                config.cosmos_chain = Some(config.cosmos_chain.clone().unwrap());
            } else {
                config.cosmos_chain = None;
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                config.chain = Some(config.chain.clone().unwrap());
            } else {
                config.chain = None;
            }
        }

        let ctx = AppContext::new();

        let dispatcher = Arc::new(CoreDispatcher::new_core(&config).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = config.clone();
            move || {
                wavs::run_server(ctx, config, dispatcher);
            }
        });

        let _aggregator_handle = std::thread::spawn({
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

                        // if wasm_digest isn't set, upload our wasm blob for permissions
                        let permissions_wasm_digest =
                            std::env::var("WAVS_E2E_PERMISSIONS_WASM_DIGEST");

                        let permissions_wasm_digest: Digest = match permissions_wasm_digest {
                            Ok(digest) => digest.parse().unwrap(),
                            Err(_) => {
                                let wasm_bytes =
                                    include_bytes!("../../../components/permissions.wasm");
                                http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
                            }
                        };

                        let hello_world_wasm_digest =
                            std::env::var("WAVS_E2E_HELLO_WORLD_WASM_DIGEST");

                        let hello_world_wasm_digest: Digest = match hello_world_wasm_digest {
                            Ok(digest) => digest.parse().unwrap(),
                            Err(_) => {
                                let wasm_bytes =
                                    include_bytes!("../../../components/hello_world.wasm");
                                http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
                            }
                        };

                        match (config.cosmos_chain.is_some(), config.chain.is_some()) {
                            (true, false) => {
                                run_tests_cosmos(http_client, config, permissions_wasm_digest).await
                            }
                            (false, true) => {
                                run_tests_ethereum(
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil.unwrap(),
                                    http_client,
                                    config,
                                    hello_world_wasm_digest,
                                )
                                .await
                            }
                            (true, true) => {
                                run_tests_crosschain(http_client, config, permissions_wasm_digest)
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
        _aggregator_handle.join().unwrap();
    }

    async fn run_tests_ethereum(
        anvil: AnvilInstance,
        http_client: HttpClient,
        config: Config,
        wasm_digest: Digest,
    ) {
        tracing::info!("Running e2e ethereum tests");

        let app = EthTestApp::new(config, anvil).await;

        let service1_id = ServiceID::new("test-1-service").unwrap();
        let service2_id = ServiceID::new("test-2-service").unwrap();

        http_client
            .create_service(
                service1_id.clone(),
                wasm_digest.clone(),
                Address::Eth(AddrEth::new(
                    app.avs_client
                        .hello_world
                        .hello_world_service_manager
                        .into(),
                )),
                Submit::EthSignedMessage { hd_index: 0 },
            )
            .await
            .unwrap();
        tracing::info!("Service created: {}, submitting task...", service1_id);

        http_client
            .create_service(
                service2_id.clone(),
                wasm_digest,
                Address::Eth(AddrEth::new(
                    app.avs_client
                        .hello_world
                        .hello_world_service_manager
                        .into(),
                )),
                Submit::EthAggregatorTx {},
            )
            .await
            .unwrap();
        tracing::info!("Service created: {}, submitting task...", service2_id);

        let avs_simple_client = app.avs_client.into_simple();
        let task1_index = avs_simple_client
            .create_new_task("foo".to_owned())
            .await
            .unwrap()
            .taskIndex;
        let task2_index = avs_simple_client
            .create_new_task("bar".to_owned())
            .await
            .unwrap()
            .taskIndex;

        tokio::time::timeout(Duration::from_secs(10), async move {
            loop {
                let (task1_response_hash, task2_response_hash) = (
                    avs_simple_client
                        .task_responded_hash(task1_index)
                        .await
                        .unwrap(),
                    avs_simple_client
                        .task_responded_hash(task2_index)
                        .await
                        .unwrap(),
                );
                if !task1_response_hash.is_empty() && !task2_response_hash.is_empty() {
                    assert_ne!(task1_response_hash, task2_response_hash);
                    tracing::info!("GOT THE TASKS RESPONSE HASH!");
                    tracing::info!("foo: {}", hex::encode(task1_response_hash));
                    tracing::info!("bar: {}", hex::encode(task2_response_hash));
                    break;
                } else {
                    tracing::info!(
                        "Waiting for task response by {} on {} for indexes {:?}...",
                        avs_simple_client.eth.address(),
                        avs_simple_client.contract_address,
                        [task1_index, task2_index]
                    );
                }
                // still open, waiting...
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
    }

    async fn run_tests_crosschain(_http_client: HttpClient, _config: Config, _wasm_digest: Digest) {
        tracing::info!("Running e2e crosschain tests");
        // TODO!
    }

    async fn run_tests_cosmos(http_client: HttpClient, config: Config, wasm_digest: Digest) {
        tracing::info!("Running e2e cosmos tests");

        let app = CosmosTestApp::new(config).await;

        let service_id = ServiceID::new("test-service").unwrap();

        http_client
            .create_service(
                service_id.clone(),
                wasm_digest,
                app.task_queue.addr.clone(),
                Submit::LayerVerifierTx {
                    hd_index: 0,
                    verifier_addr: app.verifier_addr.clone(),
                },
            )
            .await
            .unwrap();

        tracing::info!("Service created: {}", service_id);

        let tx_resp = app
            .task_queue
            .submit_task(
                "example request",
                PermissionsExampleRequest {
                    url: "https://httpbin.org/get".to_string(),
                },
            )
            .await
            .unwrap();

        let event: TaskCreatedEvent = CosmosTxEvents::from(&tx_resp)
            .event_first_by_type(TaskCreatedEvent::NAME)
            .map(cosmwasm_std::Event::from)
            .unwrap()
            .try_into()
            .unwrap();

        tracing::info!("Task created: {}", event.task_id);

        let timeout = tokio::time::timeout(Duration::from_secs(3), async move {
            loop {
                let task = app.task_queue.query_task(event.task_id).await.unwrap();
                match task.status {
                    task_queue::Status::Open {} => {
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    task_queue::Status::Completed { .. } => return Ok(task),
                    task_queue::Status::Expired {} => bail!("Task expired"),
                }
            }
        })
        .await;

        match timeout {
            Ok(task) => {
                let task = task.unwrap();
                let result = task.result.unwrap();
                tracing::info!("task completed!");
                tracing::info!("result: {:#?}", result);
            }
            Err(_) => panic!("Timeout waiting for task to complete"),
        }

        tracing::info!("regular task submission past, running test service..");

        let result: PermissionsExampleResponse = http_client
            .test_service(
                &service_id,
                PermissionsExampleRequest {
                    url: "https://httpbin.org/get".to_string(),
                },
            )
            .await
            .unwrap();

        tracing::info!("success!");
        assert!(result.filecount > 0);
        tracing::info!("{:#?}", result);
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct PermissionsExampleRequest {
        pub url: String,
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct PermissionsExampleResponse {
        pub filename: PathBuf,
        pub contents: String,
        pub filecount: usize,
    }
}
