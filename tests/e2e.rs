// Currently - e2e tests are disabled by default.
// they also assume some environment variables are set:
// MATIC_E2E_SEED_PHRASE: seed phrase for client running the tests
// MATIC_E2E_TASK_QUEUE_ADDR: address of the task queue contract

#[cfg(feature = "e2e_tests")]
mod e2e {
    use std::{sync::Arc, time::Duration};

    use anyhow::{bail, Context, Result};
    use lavs_apis::{
        events::{task_queue_events::TaskCreatedEvent, traits::TypedEvent},
        id::TaskId,
        tasks as task_queue,
    };
    use layer_climb::{prelude::*, proto::abci::TxResponse};
    use serde::Serialize;
    use wasmatic::{apis::dispatcher::Permissions, test_utils::app::TestApp};
    use wasmatic::{
        apis::Trigger,
        config::Config,
        context::AppContext,
        dispatcher::CoreDispatcher,
        http::{
            handlers::service::{add::RegisterAppRequest, upload::UploadServiceResponse},
            types::app::App,
        },
        Digest,
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

        let wasmatic_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = config.clone();
            move || {
                wasmatic::run_server(ctx, config, dispatcher);
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
        wasmatic_handle.join().unwrap();
    }

    async fn run_tests(config: Config) {
        let http_client = HttpClient::new(&config);
        // sanity test - is web service running
        let _ = http_client.get_config().await.unwrap();

        // get all env vars
        let seed_phrase = std::env::var("MATIC_E2E_MNEMONIC").expect("MATIC_E2E_MNEMONIC not set");
        let task_queue_addr = std::env::var("MATIC_E2E_TASK_QUEUE_ADDRESS")
            .expect("MATIC_E2E_TASK_QUEUE_ADDRESS not set");
        let wasm_digest = std::env::var("MATIC_E2E_WASM_DIGEST");

        // if wasm_digest isn't set, upload our wasm blob for square
        let wasm_digest: Digest = match wasm_digest {
            Ok(digest) => digest.parse().unwrap(),
            Err(_) => {
                let wasm_bytes = include_bytes!("../components/square.wasm");
                http_client.upload_wasm(wasm_bytes.to_vec()).await.unwrap()
            }
        };

        tracing::info!("Wasm digest: {}", wasm_digest);

        let chain_config = config.chain_config().unwrap();

        let key_signer = KeySigner::new_mnemonic_str(&seed_phrase, None).unwrap();
        let signing_client = SigningClient::new(chain_config.clone(), key_signer)
            .await
            .unwrap();

        let task_queue_addr = chain_config.parse_address(&task_queue_addr).unwrap();
        let task_queue = TaskQueueContract::new(signing_client.clone(), task_queue_addr)
            .await
            .unwrap();

        tracing::info!("Running tasks on task queue contract: {}", task_queue.addr);

        let _ = http_client
            .create_service("test-service", wasm_digest, &task_queue.addr)
            .await
            .unwrap();

        let tx_resp = task_queue
            .submit_task("squaring 3", serde_json::json!({ "x": 3 }))
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

                let y = result.get("y").unwrap().as_u64().unwrap();
                assert_eq!(y, 9);
            }
            Err(_) => panic!("Timeout waiting for task to complete"),
        }
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
            name: impl ToString,
            digest: Digest,
            task_queue_addr: &Address,
        ) -> Result<()> {
            let app = App {
                trigger: Trigger::Queue {
                    task_queue_addr: task_queue_addr.to_string(),
                    poll_interval: 1000,
                },
                name: name.to_string(),
                status: None,
                digest,
                permissions: Permissions::default(),
                envs: Vec::new(),
                testable: Some(true),
            };

            let body = serde_json::to_string(&RegisterAppRequest {
                app,
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

        pub async fn upload_wasm(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
            let response: UploadServiceResponse = self
                .inner
                .post(&format!("{}/upload", self.endpoint))
                .body(wasm_bytes)
                .send()
                .await?
                .json()
                .await?;

            Ok(response.digest)
        }
    }
}
