use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};

use utils::{config::CosmosChainConfig, context::AppContext, filesystem::workspace_path};

const IC_API_URL: &str = "http://127.0.0.1:8080";

pub fn start_chains(ctx: AppContext) -> Vec<(CosmosChainConfig, Option<IcTestHandle>)> {
    let mut chains = Vec::new();

    cfg_if::cfg_if! {
        if #[cfg(feature = "cosmos")] {
            chains.push(start_chain(ctx.clone(), 0));

            // cfg_if::cfg_if! {
            //     if #[cfg(feature = "aggregator")] {
            //         cosmos_chains.push(start_chain(ctx.clone(), 1));
            //     }
            // }
        }
    }

    chains
}

#[allow(dead_code)]
fn start_chain(ctx: AppContext, index: u8) -> (CosmosChainConfig, Option<IcTestHandle>) {
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
        grpc_endpoint: None,
        gas_price: 0.025,
        gas_denom: "ujuno".to_string(),
        bech32_prefix: "juno".to_string(),
        faucet_endpoint: None,
    };

    (config, ic_test_handle)
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
