use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result};
use layer_climb::{prelude::*, proto::abci::TxResponse, signing::SigningClient};
use serde::Serialize;
use wavs::{config::Config, AppContext};

use super::{http::HttpClient, wavs_path, workspace_path, Digests, ServiceIds};

const IC_API_URL: &str = "http://127.0.0.1:8080";

#[allow(dead_code)]
pub struct CosmosTestApp {
    pub signing_client: SigningClient,
}

impl CosmosTestApp {
    pub async fn new(config: Config) -> Self {
        // get all env vars
        let seed_phrase =
            std::env::var("WAVS_E2E_COSMOS_MNEMONIC").expect("WAVS_E2E_COSMOS_MNEMONIC not set");
        let key_signer = KeySigner::new_mnemonic_str(&seed_phrase, None).unwrap();

        let chain_config: ChainConfig = config.cosmos_chain_config().unwrap().clone().into();
        let signing_client = SigningClient::new(chain_config.clone(), key_signer, None)
            .await
            .unwrap();

        tracing::info!("Cosmos signing client: {}", signing_client.addr);

        Self { signing_client }
    }
}

pub fn start_chain(ctx: AppContext) {
    let ic_test_handle = IcTestHandle::spawn();

    let builder = localic_std::transactions::ChainRequestBuilder::new(
        IC_API_URL.to_string(),
        "localjuno-1".to_string(),
        true,
    )
    .unwrap();
    ctx.rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(150), async {
            let client = reqwest::Client::new();
            loop {
                if client.get(IC_API_URL).send().await.is_ok() {
                    return anyhow::Ok(());
                } else {
                    tracing::info!("Waiting for server to start...");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    });

    let chain = localic_std::node::Chain::new(&builder);
    let chain_config = chain.get_chain_config();
    tracing::info!("{:?}", chain_config);
}

/// A wrapper around a Child process that kills it when dropped.
pub struct IcTestHandle {
    child: Child,
}

impl IcTestHandle {
    /// Spawns a new process, returning a guard that will kill it when dropped.
    pub fn spawn() -> Self {
        let bin_path = std::env::var("WAVS_IC_BIN_PATH").expect("WAVS_IC_BIN_PATH not set");
        let bin_path = shellexpand::tilde(&bin_path).to_string();
        let data_path = workspace_path()
            .join("packages")
            .join("layer-tests")
            .join("interchain");
        let child = Command::new(bin_path)
            .args(["start", "juno", "--api-port", "8080"])
            .env("ICTEST_HOME", data_path)
            // If you want to see the process output in your terminal, remove this or
            // use Stdio::inherit() for stdout/stderr. Here we just discard it.
            // .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .spawn()
            .unwrap();

        Self { child }
    }
}

impl Drop for IcTestHandle {
    fn drop(&mut self) {
        // Attempt to kill the child process. Ignore errors if it's already dead.
        let _ = self.child.kill();
        // We can wait on it to ensure it has actually terminated.
        let _ = self.child.wait();
    }
}

pub async fn run_tests_cosmos(
    http_client: HttpClient,
    config: Config,
    digests: Digests,
    service_ids: ServiceIds,
) {
    /*
    tracing::info!("Running e2e cosmos tests");

    let app = CosmosTestApp::new(config).await;

    if let Some(service_id) = service_ids.cosmos_permissions() {
        let wasm_digest = digests.permissions_digest().await.unwrap();

        http_client
            .create_service(
                service_id.clone(),
                wasm_digest,
                Trigger::contract_event(app.task_queue.addr.clone()),
                Submit::CosmosContract { chain_name: "foo".to_string(), contract_addr: app.verifier_addr.clone() },
                ComponentWorld::ChainEvent,
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
    */
}
