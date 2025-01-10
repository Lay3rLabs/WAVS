use anyhow::{Context, Result};
use layer_climb::{prelude::*, proto::abci::TxResponse, signing::SigningClient};
use serde::Serialize;
use wavs::config::Config;

use super::{http::HttpClient, Digests, ServiceIds};

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
