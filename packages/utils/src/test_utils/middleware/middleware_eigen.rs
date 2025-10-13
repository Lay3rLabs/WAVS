use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use anyhow::{bail, Result};
use serde::Deserialize;
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;

use super::{
    middleware_config_filename, middleware_deploy_filename, MiddlewareServiceManager,
    MiddlewareServiceManagerConfig, MIDDLEWARE_IMAGE,
};

pub struct EigenlayerMiddleware {
    pub container_id: String,
    nodes_dir: TempDir,
    config_dir: TempDir,
    service_manager_count: AtomicUsize,
}

impl EigenlayerMiddleware {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub async fn new() -> Result<Self> {
        let nodes_dir = TempDir::new()?;
        let config_dir = TempDir::new()?;

        let output = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "run",
                    "-d",
                    "--network",
                    "host",
                    "--entrypoint",
                    "",
                    "-v",
                    &format!("{}:/root/.nodes", nodes_dir.path().display()),
                    "-v",
                    &format!(
                        "{}:/wavs/contracts/deployments",
                        config_dir.path().display()
                    ),
                    MIDDLEWARE_IMAGE,
                    "tail",
                    "-f",
                    "/dev/null",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
                .wait_with_output(),
        )
        .await??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "Failed to start EigenLayer middleware container: {}",
                stderr
            );
        }

        let container_id = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("Failed to read container ID: {}", e))?
            .trim()
            .to_string();

        if container_id.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Docker returned empty container ID. stderr: {}", stderr);
        }

        Ok(Self {
            container_id,
            nodes_dir,
            config_dir,
            service_manager_count: AtomicUsize::new(0),
        })
    }

    pub async fn deploy_service_manager(
        &self,
        rpc_url: String,
        deployer_key_hex: String,
    ) -> Result<MiddlewareServiceManager> {
        let id = self
            .service_manager_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_string();

        let filename = middleware_deploy_filename(&id);

        // https://github.com/Lay3rLabs/wavs-middleware?tab=readme-ov-file#2-deploy-empty-mock-contracts
        let output = tokio::time::timeout(Self::DEFAULT_TIMEOUT, async {
            let res = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("MOCK_DEPLOYER_KEY={deployer_key_hex}"),
                    "-e",
                    &format!("MOCK_RPC_URL={rpc_url}"),
                    "-e",
                    &format!("DEPLOY_FILE_MOCK={filename}"),
                    &self.container_id,
                    "/wavs/scripts/cli.sh",
                    "-m",
                    "mock",
                    "deploy",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait()
                .await?;

            if !res.success() {
                bail!("Failed to deploy service manager");
            }

            // wait for file to land
            loop {
                let output =
                    fs::read_to_string(self.nodes_dir.path().join(format!("{filename}.json")))
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to read service manager JSON: {}", e));
                if output.is_ok() {
                    break output;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await??;

        #[derive(Deserialize)]
        struct DeploymentJson {
            addresses: MiddlewareServiceManager,
        }

        let mut deployment_json: DeploymentJson = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse service manager JSON: {}", e))?;

        deployment_json.addresses.deployer_key_hex = deployer_key_hex;
        deployment_json.addresses.rpc_url = rpc_url;
        deployment_json.addresses.id = id;

        Ok(deployment_json.addresses)
    }

    pub async fn configure_service_manager(
        &self,
        service_manager: &MiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        let filename = middleware_config_filename(&service_manager.id);
        let config_filepath = self.config_dir.path().join(format!("{filename}.json"));
        fs::write(&config_filepath, serde_json::to_string(config)?).await?;

        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("MOCK_DEPLOYER_KEY={}", service_manager.deployer_key_hex),
                    "-e",
                    &format!("MOCK_RPC_URL={}", service_manager.rpc_url),
                    "-e",
                    &format!("MOCK_SERVICE_MANAGER_ADDRESS={}", service_manager.address),
                    "-e",
                    &format!("CONFIGURE_FILE={}", filename),
                    &self.container_id,
                    "/wavs/scripts/cli.sh",
                    "-m",
                    "mock",
                    "configure",
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !res.success() {
            bail!("Failed to configure service manager");
        }

        Ok(())
    }

    pub async fn set_service_manager_uri(
        &self,
        service_manager: &MiddlewareServiceManager,
        service_uri: &str,
    ) -> Result<()> {
        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("RPC_URL={}", service_manager.rpc_url),
                    "-e",
                    &format!("WAVS_SERVICE_MANAGER_ADDRESS={}", service_manager.address),
                    "-e",
                    &format!("FUNDED_KEY={}", service_manager.deployer_key_hex),
                    "-e",
                    &format!("SERVICE_URI={}", service_uri),
                    &self.container_id,
                    "/wavs/scripts/cli.sh",
                    "set_service_uri",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !res.success() {
            bail!("Failed to set service URI");
        }

        Ok(())
    }
}

impl Drop for EigenlayerMiddleware {
    fn drop(&mut self) {
        tracing::warn!(
            "Stopping EigenLayer middleware container: {}",
            self.container_id
        );
        if let Err(e) = std::process::Command::new("docker")
            .args(["rm", "-f", &self.container_id])
            .spawn()
            .and_then(|mut cmd| cmd.wait())
        {
            tracing::warn!("Failed to remove EigenLayer middleware container: {:?}", e);
        }
    }
}
