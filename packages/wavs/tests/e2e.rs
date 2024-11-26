// Currently - e2e tests are disabled by default.
// they also assume some environment variables are set:
// MATIC_E2E_SEED_PHRASE: seed phrase for client running the tests
// MATIC_E2E_TASK_QUEUE_ADDR: address of the task queue contract

#[cfg(feature = "e2e_tests")]
mod e2e {
    use std::{path::PathBuf, sync::Arc, time::Duration};

    use anyhow::{bail, Context, Result};
    use lavs_apis::{
        events::{task_queue_events::TaskCreatedEvent, traits::TypedEvent},
        id::TaskId,
        tasks as task_queue,
    };
    use layer_climb::{prelude::*, proto::abci::TxResponse};
    use serde::{de::DeserializeOwned, Deserialize, Serialize};
    use wavs::{
        apis::dispatcher::AllowedHostPermission,
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
        let config = {
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
                        run_tests(config).await;
                        ctx.kill();
                    }
                });
            }
        });

        test_handle.join().unwrap();
        wavs_handle.join().unwrap();
    }

    async fn run_tests(config: Config) {
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

        // get all env vars
        let seed_phrase = std::env::var("MATIC_E2E_MNEMONIC").expect("MATIC_E2E_MNEMONIC not set");
        let task_queue_addr = std::env::var("MATIC_E2E_TASK_QUEUE_ADDRESS")
            .expect("MATIC_E2E_TASK_QUEUE_ADDRESS not set");
        let wasm_digest = std::env::var("MATIC_E2E_WASM_DIGEST");

        // if wasm_digest isn't set, upload our wasm blob for square
        let wasm_digest: Digest = match wasm_digest {
            Ok(digest) => digest.parse().unwrap(),
            Err(_) => {
                let wasm_bytes = include_bytes!("../../../components/permissions.wasm");
                http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
            }
        };

        tracing::info!("Wasm digest: {}", wasm_digest);

        let chain_config = config.chain_config().unwrap();

        let key_signer = KeySigner::new_mnemonic_str(&seed_phrase, None).unwrap();
        let signing_client = SigningClient::new(chain_config.clone(), key_signer)
            .await
            .unwrap();

        tracing::info!(
            "Creating service on task queue contract: {}",
            task_queue_addr
        );
        let task_queue_addr = chain_config.parse_address(&task_queue_addr).unwrap();

        let task_queue = TaskQueueContract::new(signing_client.clone(), task_queue_addr)
            .await
            .unwrap();

        let service_id = ID::new("test-service").unwrap();

        let _ = http_client
            .create_service(service_id.clone(), wasm_digest, &task_queue.addr)
            .await
            .unwrap();

        tracing::info!("Service created: {}", service_id);

        let tx_resp = task_queue
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
                let task = task_queue.query_task(event.task_id).await.unwrap();
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

    struct TaskQueueContract {
        pub client: SigningClient,
        pub addr: Address,
        pub _verifier: VerifierContract,
        pub task_cost: Option<Coin>,
    }

    impl TaskQueueContract {
        pub async fn new(client: SigningClient, addr: Address) -> Result<Self> {
            let resp: task_queue::ConfigResponse = client
                .querier
                .contract_smart(
                    &addr,
                    &task_queue::QueryMsg::Custom(task_queue::CustomQueryMsg::Config {}),
                )
                .await?;

            let task_cost = match resp.requestor {
                task_queue::Requestor::Fixed(_) => None,
                task_queue::Requestor::OpenPayment(coin) => Some(new_coin(coin.amount, coin.denom)),
            };

            let verifier = VerifierContract::new(
                client.clone(),
                client.querier.chain_config.parse_address(&resp.verifier)?,
            )
            .await?;

            Ok(Self {
                client,
                addr,
                _verifier: verifier,
                task_cost,
            })
        }

        pub async fn submit_task(
            &self,
            description: impl ToString,
            payload: impl Serialize,
        ) -> Result<TxResponse> {
            let msg = task_queue::ExecuteMsg::Custom(task_queue::CustomExecuteMsg::Create {
                description: description.to_string(),
                timeout: None,
                payload: serde_json::to_value(payload)?,
                with_completed_hooks: None,
                with_timeout_hooks: None,
            });

            let funds = match self.task_cost.as_ref() {
                Some(cost) => vec![cost.clone()],
                None => vec![],
            };

            self.client
                .contract_execute(&self.addr, &msg, funds, None)
                .await
                .context("submit task")
        }

        pub async fn query_task(&self, id: TaskId) -> Result<task_queue::TaskResponse> {
            self.client
                .querier
                .contract_smart(
                    &self.addr,
                    &task_queue::QueryMsg::Custom(task_queue::CustomQueryMsg::Task { id }),
                )
                .await
                .context("query task")
        }
    }

    struct VerifierContract {
        pub _client: SigningClient,
        pub _addr: Address,
    }

    impl VerifierContract {
        pub async fn new(client: SigningClient, addr: Address) -> Result<Self> {
            Ok(Self {
                _client: client,
                _addr: addr,
            })
        }
    }

    struct HttpClient {
        inner: reqwest::Client,
        endpoint: String,
    }

    impl HttpClient {
        pub fn new(config: &Config) -> Self {
            let endpoint = format!("http://{}:{}", config.host, config.port);

            Self {
                inner: reqwest::Client::new(),
                endpoint,
            }
        }

        pub async fn get_config(&self) -> Result<Config> {
            self.inner
                .get(&format!("{}/config", self.endpoint))
                .send()
                .await?
                .json()
                .await
                .map_err(|e| e.into())
        }

        pub async fn create_service(
            &self,
            id: ID,
            digest: Digest,
            task_queue_addr: impl ToString,
        ) -> Result<()> {
            let service = ServiceRequest {
                trigger: TriggerRequest::queue(task_queue_addr, 1000, 0),
                id,
                digest: digest.into(),
                permissions: Permissions {
                    allowed_http_hosts: AllowedHostPermission::All,
                    file_system: true,
                },
                envs: Vec::new(),
                testable: Some(true),
            };

            let body = serde_json::to_string(&AddServiceRequest {
                service,
                wasm_url: None,
            })?;

            self.inner
                .post(&format!("{}/app", self.endpoint))
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await?
                .error_for_status()?;

            Ok(())
        }

        pub async fn test_service<D: DeserializeOwned>(
            &self,
            name: impl ToString,
            input: impl Serialize,
        ) -> Result<D> {
            let body = serde_json::to_string(&TestAppRequest {
                name: name.to_string(),
                input: Some(serde_json::to_value(input)?),
            })?;

            let response: TestAppResponse = self
                .inner
                .post(&format!("{}/test", self.endpoint))
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await?
                .json()
                .await?;

            Ok(serde_json::from_value(response.output)?)
        }

        pub async fn upload_wasm(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
            let response: UploadServiceResponse = self
                .inner
                .post(&format!("{}/upload", self.endpoint))
                .body(wasm_bytes)
                .send()
                .await?
                .json()
                .await?;

            Ok(response.digest.into())
        }
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
