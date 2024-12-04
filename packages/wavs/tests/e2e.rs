// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

#[cfg(feature = "e2e_tests")]
mod e2e {
    mod eth;
    mod http;
    mod layer;

    use std::{path::PathBuf, sync::Arc, time::Duration};

    use alloy::node_bindings::{Anvil, AnvilInstance};
    use anyhow::{bail, Context, Result};
    use eth::EthTestApp;
    use http::HttpClient;
    use lavs_apis::{
        events::{task_queue_events::TaskCreatedEvent, traits::TypedEvent},
        id::TaskId,
        tasks as task_queue,
    };
    use layer::LayerTestApp;
    use layer_climb::{prelude::*, proto::abci::TxResponse};
    use serde::{de::DeserializeOwned, Deserialize, Serialize};
    use utils::{
        eigen_client::EigenClient,
        eth_client::{EthClientBuilder, EthClientConfig},
        hello_world::{HelloWorldClient, HelloWorldClientBuilder},
    };
    use wavs::{
        apis::{dispatcher::AllowedHostPermission, ChainKind},
        config::Config,
        context::AppContext,
        dispatcher::CoreDispatcher,
        http::handlers::service::{
            add::{AddServiceRequest, ServiceRequest},
            upload::UploadServiceResponse,
        },
        Digest,
    };
    use wavs::{
        apis::{dispatcher::Permissions, ID},
        http::{
            handlers::service::test::{TestAppRequest, TestAppResponse},
            types::TriggerRequest,
        },
        test_utils::app::TestApp,
    };

    #[test]
    fn e2e_tests() {
        let mut config = {
            tokio::runtime::Runtime::new().unwrap().block_on({
                async {
                    let mut cli_args = TestApp::default_cli_args();
                    cli_args.dotenv = None;
                    cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
                    TestApp::new_with_args(cli_args)
                        .await
                        .config
                        .as_ref()
                        .clone()
                }
            })
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_layer")] {
                config.layer_chain = Some(config.layer_chain.clone().unwrap());
            } else {
                config.layer_chain = None;
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                config.chain= Some(config.chain.clone().unwrap());
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

                        // if wasm_digest isn't set, upload our wasm blob for square
                        let wasm_digest = std::env::var("WAVS_E2E_WASM_DIGEST");

                        let wasm_digest: Digest = match wasm_digest {
                            Ok(digest) => digest.parse().unwrap(),
                            Err(_) => {
                                let wasm_bytes =
                                    include_bytes!("../../../components/permissions.wasm");
                                http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
                            }
                        };

                        match (config.layer_chain.is_some(), config.chain.is_some()) {
                            (true, false) => {
                                run_tests_layer(http_client, config, wasm_digest).await
                            }
                            (false, true) => {
                                run_tests_ethereum(http_client, config, wasm_digest).await
                            }
                            (true, true) => {
                                run_tests_crosschain(http_client, config, wasm_digest).await
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
    }

    async fn run_tests_ethereum(_http_client: HttpClient, _config: Config, _wasm_digest: Digest) {
        tracing::info!("Running e2e ethereum tests");

        let app = EthTestApp::new(_config).await;

        let new_task = app
            .avs_client
            .create_new_task("foo".to_owned())
            .await
            .unwrap();
        assert_eq!(new_task.taskIndex, 0);
        assert_eq!(new_task.task.name, "foo");
    }

    async fn run_tests_crosschain(_http_client: HttpClient, _config: Config, _wasm_digest: Digest) {
        tracing::info!("Running e2e crosschain tests");
        // TODO!
    }

    async fn run_tests_layer(http_client: HttpClient, config: Config, wasm_digest: Digest) {
        tracing::info!("Running e2e layer tests");

        let app = LayerTestApp::new(config).await;

        let service_id = ID::new("test-service").unwrap();

        let _ = http_client
            .create_service(
                service_id.clone(),
                wasm_digest,
                &app.task_queue.addr,
                ChainKind::Layer,
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
