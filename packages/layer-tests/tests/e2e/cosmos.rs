use std::{
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use cosmwasm_std::Empty;
use layer_climb::{prelude::*, proto::abci::TxResponse, signing::SigningClient};
use serde::Serialize;
use tempfile::tempfile;
use utils::config::CosmosChainConfig;
use wavs::{
    apis::{
        dispatcher::{ComponentWorld, Submit},
        trigger::Trigger,
    },
    config::Config,
    AppContext,
};

use super::{http::HttpClient, wavs_path, workspace_path, Digests, ServiceIds};

const IC_API_URL: &str = "http://127.0.0.1:8080";

#[allow(dead_code)]
pub fn start_chain(
    ctx: AppContext,
    index: u8,
) -> (String, CosmosChainConfig, Option<IcTestHandle>) {
    let mut ic_test_handle = None;

    let chain_info = ctx.rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(30), async {
            let client = reqwest::Client::new();
            let sleep_duration = Duration::from_millis(100);
            let mut log_clock = Instant::now();
            loop {
                let chain_info = match client.get(format!("{IC_API_URL}/info")).send().await {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(json) => json
                            .as_object()
                            .and_then(|json| json.get("logs"))
                            .and_then(|logs| logs.get("chains"))
                            .and_then(|logs| logs.as_array())
                            .and_then(|logs| {
                                logs.iter().find(|log| log["chain_id"] == "localjuno-1")
                            })
                            .cloned(),
                        Err(_) => None,
                    },
                    Err(_) => None,
                };

                match chain_info {
                    Some(chain_info) => {
                        return chain_info;
                    }
                    None => {
                        if ic_test_handle.is_none() {
                            ic_test_handle = Some(IcTestHandle::spawn());
                        }
                        tokio::time::sleep(sleep_duration).await;
                        if Instant::now() - log_clock > Duration::from_secs(3) {
                            tracing::info!("Waiting for server to start...");
                            log_clock = Instant::now();
                        }
                    }
                }
            }
        })
        .await
        .unwrap()
    });

    let config = CosmosChainConfig {
        chain_id: chain_info
            .get("chain_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string(),
        rpc_endpoint: chain_info
            .get("rpc_address")
            .map(|rpc| rpc.as_str().unwrap().to_string()),
        //grpc_endpoint: chain_info.get("grpc_address").map(|grpc| grpc.as_str().unwrap().to_string()),
        grpc_endpoint: None,
        gas_price: 0.025,
        gas_denom: "ujuno".to_string(),
        bech32_prefix: "juno".to_string(),
        faucet_endpoint: None,
    };

    (
        format!("local-cosmos-test-{}", index),
        config,
        ic_test_handle,
    )
}

/// A wrapper around a Child process that kills it when dropped.
pub struct IcTestHandle {
    child: Child,
    data_dir: tempfile::TempDir,
}

impl IcTestHandle {
    /// Spawns a new process, returning a guard that will kill it when dropped.
    pub fn spawn() -> Self {
        let bin_path = match std::env::var("WAVS_LOCAL_IC_BIN_PATH") {
            Ok(bin_path) => shellexpand::tilde(&bin_path).to_string(),
            Err(_) => "local-ic".to_string(),
        };
        let repo_data_path = workspace_path()
            .join("packages")
            .join("layer-tests")
            .join("interchain");

        let temp_data = tempfile::tempdir().unwrap();

        // recursively copy all files and directories from repo_data_path to data_path
        let _ = fs_extra::dir::copy(
            repo_data_path,
            temp_data.path(),
            &fs_extra::dir::CopyOptions {
                overwrite: true,
                content_only: true,
                ..Default::default()
            },
        );

        let child = Command::new(bin_path)
            .args(["start", "juno", "--api-port", "8080"])
            .env("ICTEST_HOME", temp_data.path())
            // can be more quiet by uncommenting these
            // .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .spawn()
            .unwrap();

        tracing::info!("starting LocalIc (pid {})", child.id());
        Self {
            child,
            data_dir: temp_data,
        }
    }
}

impl Drop for IcTestHandle {
    fn drop(&mut self) {
        tracing::info!("dropping IcTestHandle, killing process {}", self.child.id());
        // Attempt to kill the child process. Ignore errors if it's already dead.
        let _ = self.child.kill();
        // We can wait on it to ensure it has actually terminated.
        let _ = self.child.wait();
    }
}

#[derive(Clone)]
pub struct CosmosTestApp {
    pub signing_client: SigningClient,
    pub chain_name: String,
    pub chain_config: CosmosChainConfig,
    handle: Option<Arc<IcTestHandle>>,
}

impl CosmosTestApp {
    pub async fn new(
        chain_name: String,
        chain_config: CosmosChainConfig,
        handle: Option<IcTestHandle>,
    ) -> Self {
        // get all env vars
        let seed_phrase = "decorate bright ozone fork gallery riot bus exhaust worth way bone indoor calm squirrel merry zero scheme cotton until shop any excess stage laundry";
        let key_signer = KeySigner::new_mnemonic_str(&seed_phrase, None).unwrap();

        let climb_chain_config: ChainConfig = chain_config.clone().into();
        let signing_client = SigningClient::new(climb_chain_config, key_signer, None)
            .await
            .unwrap();

        tracing::info!("Cosmos signing client: {}", signing_client.addr);

        Self {
            chain_name,
            signing_client,
            chain_config,
            handle: handle.map(Arc::new),
        }
    }

    pub async fn deploy_contracts(&self) -> CosmosContracts {
        let contract_path = workspace_path()
            .join("artifacts")
            .join("simple_example.wasm");

        if !contract_path.exists() {
            panic!("Contract not found at {:?}", contract_path);
        }

        let wasm_byte_code = std::fs::read(contract_path).unwrap();

        let (code_id, _) = self
            .signing_client
            .contract_upload_file(wasm_byte_code, None)
            .await
            .unwrap();

        let (trigger_addr, _) = self
            .signing_client
            .contract_instantiate(
                None,
                code_id,
                "trigger".to_string(),
                &Empty {},
                vec![],
                None,
            )
            .await
            .unwrap();

        let (submit_addr, _) = self
            .signing_client
            .contract_instantiate(None, code_id, "submit".to_string(), &Empty {}, vec![], None)
            .await
            .unwrap();

        CosmosContracts {
            trigger: trigger_addr,
            submit: submit_addr,
        }
    }
}

#[derive(Debug)]
pub struct CosmosContracts {
    pub trigger: Address,
    pub submit: Address,
}

pub async fn run_tests(
    cosmos_apps: Vec<CosmosTestApp>,
    http_client: HttpClient,
    digests: Digests,
    service_ids: ServiceIds,
) {
    let app = cosmos_apps.first().unwrap();
    let contracts = app.deploy_contracts().await;

    tracing::info!("Contracts deployed: {:#?}", contracts);

    if let Some(service_id) = service_ids.cosmos_permissions {
        let wasm_digest = digests.permissions.unwrap();

        // http_client
        //     .create_service(
        //         service_id.clone(),
        //         wasm_digest,
        //         Trigger::cosmos_contract_event(contracts.trigger.clone(), app.chain_name.clone(), simple_example_cosmos::event::NewMessageEvent::KEY),
        //         Submit::eigen_contract(chain_name, service_manager, aggregate),
        //         ComponentWorld::AnyContractEvent,
        //     )
        //     .await
        //     .unwrap();

        // tracing::info!("Service created: {}", service_id);

        /*


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
        */
    }
}
