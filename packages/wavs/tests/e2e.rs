// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

#[cfg(feature = "e2e_tests")]
mod e2e {
    mod cosmos;
    mod eth;
    mod http;

    use std::{path::PathBuf, sync::Arc, time::Duration};

    #[cfg(feature = "e2e_tests_ethereum")]
    use alloy::node_bindings::Anvil;
    use alloy::node_bindings::AnvilInstance;
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
    use utils::layer_contract_client::LayerContractClientSimple;
    use wavs::{
        apis::{dispatcher::Submit, ServiceID},
        http::types::TriggerRequest,
        test_utils::app::TestApp,
    };
    use wavs::{config::Config, dispatcher::CoreDispatcher, AppContext, Digest};

    #[test]
    fn e2e_tests() {
        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                let anvil: Option<AnvilInstance> = Some(Anvil::new().port(8545u16).chain_id(31337).spawn());
                let anvil2: Option<AnvilInstance> = Some(Anvil::new().port(8645u16).chain_id(31338).spawn());
            } else {
                let anvil: Option<AnvilInstance> = None;
                let anvil2: Option<AnvilInstance> = None;
            }
        }
        let mut config: Config = {
            tokio::runtime::Runtime::new().unwrap().block_on({
                async {
                    let mut cli_args = TestApp::default_cli_args();
                    cli_args.dotenv = None;
                    cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
                    // parent directory for the default wavs.toml
                    cli_args.home = Some(PathBuf::from(".."));
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
            cli_args.home = Some(PathBuf::from("..").join("aggregator"));
            cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
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
                config.enabled_ethereum = vec!["local".to_string(), "e2elocal2".to_string()];
                if let Err(e) = config.ethereum_chain_configs() {
                    tracing::debug!("ethereum_chain_configs: {:?}", config);
                    panic!("Error in ethereum_chain_configs: {}", e);
                }
            } else {
                config.enabled_ethereum = vec![];
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

                        let eth_trigger_echo_wasm_digest =
                            std::env::var("WAVS_E2E_ETH_TRIGGER_ECHO_WASM_DIGEST");

                        let eth_trigger_echo_wasm_digest: Digest =
                            match eth_trigger_echo_wasm_digest {
                                Ok(digest) => digest.parse().unwrap(),
                                Err(_) => {
                                    let wasm_bytes =
                                        include_bytes!("../../../components/eth_trigger_echo.wasm");
                                    http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
                                }
                            };

                        let eth_trigger_square_wasm_digest =
                            std::env::var("WAVS_E2E_ETH_TRIGGER_SQUARE_WASM_DIGEST");

                        let eth_trigger_square_wasm_digest: Digest =
                            match eth_trigger_square_wasm_digest {
                                Ok(digest) => digest.parse().unwrap(),
                                Err(_) => {
                                    let wasm_bytes = include_bytes!(
                                        "../../../components/eth_trigger_square.wasm"
                                    );
                                    http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
                                }
                            };

                        let eth_chain = config.enabled_ethereum.first();

                        match (config.cosmos_chain.is_some(), eth_chain.is_some()) {
                            (true, false) => {
                                run_tests_cosmos(http_client, config, permissions_wasm_digest).await
                            }
                            (false, true) => {
                                run_tests_ethereum(
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil.unwrap(),
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil2.unwrap(),
                                    http_client,
                                    config,
                                    eth_trigger_echo_wasm_digest,
                                    eth_trigger_square_wasm_digest,
                                )
                                .await;
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
        aggregator_handle.join().unwrap();
    }

    async fn run_tests_ethereum(
        anvil: AnvilInstance,
        anvil2: AnvilInstance,
        http_client: HttpClient,
        config: Config,
        echo_wasm_digest: Digest,
        square_wasm_digest: Digest,
    ) {
        tracing::info!("Running e2e ethereum tests");

        let app = EthTestApp::new(config.clone(), anvil).await;
        let app2 = EthTestApp::new(config.clone(), anvil2).await;

        let square_service_id = ServiceID::new("square-service").unwrap();
        let echo_service_id = ServiceID::new("echo-service").unwrap();

        let square_service_id2 = ServiceID::new("square-service2").unwrap();
        let echo_service_id2 = ServiceID::new("echo-service2").unwrap();

        eth_create_services(
            &http_client,
            &app,
            echo_service_id.clone(),
            square_service_id.clone(),
            echo_wasm_digest.clone(),
            square_wasm_digest.clone(),
        )
        .await;
        eth_create_services(
            &http_client,
            &app2,
            echo_service_id2.clone(),
            square_service_id2.clone(),
            echo_wasm_digest.clone(),
            square_wasm_digest.clone(),
        )
        .await;

        let avs_simple_client: LayerContractClientSimple = app.avs_client.clone().into();
        let avs_simple_client2: LayerContractClientSimple = app2.avs_client.clone().into();

        eth_submit_tasks(echo_service_id, square_service_id, &avs_simple_client).await;
        eth_submit_tasks(echo_service_id2, square_service_id2, &avs_simple_client2).await;

        // TODO - now with aggregator and multiple payloads within a service....
        eth_verify_triggers(
            &http_client,
            &app,
            &config,
            ServiceID::new("echo-aggregate-service").unwrap(),
            echo_wasm_digest.clone(),
            &avs_simple_client,
        )
        .await;
        eth_verify_triggers(
            &http_client,
            &app2,
            &config,
            ServiceID::new("echo-aggregate-service-2").unwrap(),
            echo_wasm_digest.clone(),
            &avs_simple_client2,
        )
        .await;
    }

    async fn eth_create_services(
        http_client: &HttpClient,
        app: &EthTestApp,
        echo_service_id: ServiceID,
        square_service_id: ServiceID,
        echo_wasm_digest: Digest,
        square_wasm_digest: Digest,
    ) {
        let chain_id = app.chain_id();
        http_client
            .create_service(
                echo_service_id.clone(),
                echo_wasm_digest.clone(),
                TriggerRequest::eth_event(Address::Eth(AddrEth::new(
                    app.avs_client.layer.trigger.into(),
                ))),
                Submit::EthSignedMessage {
                    chain_id: chain_id.to_string(),
                    hd_index: 0,
                    service_manager_addr: Address::Eth(AddrEth::new(
                        app.avs_client.layer.service_manager.into(),
                    )),
                },
            )
            .await
            .unwrap();
        tracing::info!("Service created: {}", echo_service_id);

        http_client
            .create_service(
                square_service_id.clone(),
                square_wasm_digest,
                TriggerRequest::eth_event(Address::Eth(AddrEth::new(
                    app.avs_client.layer.trigger.into(),
                ))),
                Submit::EthSignedMessage {
                    chain_id: chain_id.to_string(),
                    hd_index: 0,
                    service_manager_addr: Address::Eth(AddrEth::new(
                        app.avs_client.layer.service_manager.into(),
                    )),
                },
            )
            .await
            .unwrap();
        tracing::info!(
            "(chain_id:{}) Service created: {}",
            app.chain_id(),
            square_service_id
        );
    }

    async fn eth_submit_tasks(
        echo_service_id: ServiceID,
        square_service_id: ServiceID,
        avs_simple_client: &LayerContractClientSimple,
    ) {
        tracing::info!(
            "(chain_id:{}) Submitting echo task...",
            avs_simple_client.eth.config.chain_id
        );
        let echo_trigger_id = avs_simple_client
            .trigger
            .add_trigger(
                echo_service_id.to_string(),
                "default".to_string(),
                b"foo".to_vec(),
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let avs_simple_client = avs_simple_client.clone();
            async move {
                loop {
                    let signed_data = avs_simple_client
                        .load_signed_data(echo_trigger_id)
                        .await
                        .unwrap();
                    match signed_data {
                        Some(signed_data) => {
                            tracing::info!("(chain_id:{}) GOT THE SIGNATURE!", avs_simple_client.eth.config.chain_id);
                            tracing::info!("{}", hex::encode(signed_data.signature));
                            break;
                        }
                        None => {
                            tracing::info!(
                                "(chain_id:{}) Waiting for task response by {} on {} for trigger_id {}...",
                                avs_simple_client.eth.config.chain_id,
                                avs_simple_client.eth.address(),
                                avs_simple_client.service_manager_contract_address,
                                echo_trigger_id
                            );
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();

        tracing::info!(
            "(chain_id:{}) Submitting square task...",
            avs_simple_client.eth.config.chain_id
        );
        let square_trigger_id = avs_simple_client
            .trigger
            .add_trigger(
                square_service_id.to_string(),
                "default".to_string(),
                serde_json::to_vec(&SquareRequest { x: 3 }).unwrap(),
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let avs_simple_client = avs_simple_client.clone();
            async move {
                loop {
                    let signed_data = avs_simple_client
                        .load_signed_data(square_trigger_id)
                        .await
                        .unwrap();
                    match signed_data {
                        Some(signed_data) => {
                            tracing::info!("GOT THE SIGNATURE!");
                            tracing::info!("{}", hex::encode(signed_data.signature));

                            let response =
                                serde_json::from_slice::<SquareResponse>(&signed_data.data)
                                    .unwrap();

                            tracing::info!(
                                "(chain_id:{}) GOT THE RESPONSE!",
                                avs_simple_client.eth.config.chain_id
                            );
                            tracing::info!("{:?}", response);
                            break;
                        }
                        None => {
                            tracing::info!(
                                "Waiting for task response by {} on {} for trigger_id {}...",
                                avs_simple_client.eth.address(),
                                avs_simple_client.service_manager_contract_address,
                                square_trigger_id
                            );
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }

    async fn eth_verify_triggers(
        http_client: &HttpClient,
        app: &EthTestApp,
        config: &Config,
        echo_aggregate_service_id: ServiceID,
        echo_wasm_digest: Digest,
        avs_simple_client: &LayerContractClientSimple,
    ) {
        http_client
            .create_service(
                echo_aggregate_service_id.clone(),
                echo_wasm_digest,
                TriggerRequest::eth_event(Address::Eth(AddrEth::new(
                    avs_simple_client.trigger.contract_address.into(),
                ))),
                Submit::EthAggregatorTx {
                    chain_id: app.chain_id(),
                    service_manager_addr: Address::Eth(AddrEth::new(
                        avs_simple_client.service_manager_contract_address.into(),
                    )),
                },
            )
            .await
            .unwrap();
        tracing::info!(
            "(chain_id:{}) Service created: {}",
            app.chain_id(),
            echo_aggregate_service_id
        );

        http_client
            .register_service_on_aggregator(
                avs_simple_client.service_manager_contract_address,
                echo_aggregate_service_id.clone(),
                app.chain_id(),
                config,
            )
            .await
            .unwrap();

        let echo_aggregate_trigger_id_1 = avs_simple_client
            .trigger
            .add_trigger(
                echo_aggregate_service_id.to_string(),
                "default".to_string(),
                b"foo-aggregate".to_vec(),
            )
            .await
            .unwrap();

        let echo_aggregate_trigger_id_2 = avs_simple_client
            .trigger
            .add_trigger(
                echo_aggregate_service_id.to_string(),
                "default".to_string(),
                b"bar-aggregate".to_vec(),
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let avs_simple_client = avs_simple_client.clone();
            async move {
                loop {
                    let signed_data_1 = avs_simple_client
                        .load_signed_data(echo_aggregate_trigger_id_1)
                        .await
                        .unwrap();

                    let signed_data_2 = avs_simple_client
                        .load_signed_data(echo_aggregate_trigger_id_2)
                        .await
                        .unwrap();

                    match (signed_data_1, signed_data_2) {
                        (Some(signed_data_1), Some(signed_data_2)) => {
                            tracing::info!(
                                "(chain_id:{}) GOT THE SIGNATURES!",
                                avs_simple_client.eth.config.chain_id
                            );
                            tracing::info!("1: {}", hex::encode(signed_data_1.signature));
                            tracing::info!("2: {}", hex::encode(signed_data_2.signature));
                            break;
                        }
                        (None, Some(_)) => {
                            tracing::info!("Got aggregation #1, waiting for #2...");
                        }
                        (Some(_), None) => {
                            tracing::info!("Got aggregation #2, waiting for #1...");
                        }
                        (None, None) => {
                            tracing::info!("Waiting for aggregation responses...");
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
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
                TriggerRequest::LayerQueue {
                    task_queue_addr: app.task_queue.addr.clone(),
                    poll_interval: 1000,
                    hd_index: 0,
                },
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

    #[derive(Serialize, Debug)]
    pub struct SquareRequest {
        pub x: u64,
    }

    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    pub struct SquareResponse {
        pub y: u64,
    }
}
