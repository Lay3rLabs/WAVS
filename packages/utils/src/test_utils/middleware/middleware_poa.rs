use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use alloy_primitives::Address;
use anyhow::{bail, Result};
use serde::Deserialize;
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;

use super::{
    MiddlewareServiceManager, MiddlewareServiceManagerConfig, ANVIL_DEPLOYER_ADDRESS,
    ANVIL_DEPLOYER_KEY, POA_MIDDLEWARE_IMAGE,
};

pub struct PoaMiddleware {
    pub container_id: String,
    nodes_dir: TempDir,
    service_manager_count: AtomicUsize,
}

impl PoaMiddleware {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub async fn new() -> Result<Self> {
        let nodes_dir = TempDir::new()?;

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
                    POA_MIDDLEWARE_IMAGE,
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
            bail!("Failed to start POA middleware container: {}", stderr);
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

        let output = tokio::time::timeout(Self::DEFAULT_TIMEOUT, async {
            let res = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("FUNDED_KEY={}", deployer_key_hex),
                    "-e",
                    &format!("RPC_URL={rpc_url}"),
                    "-e",
                    "DEPLOY_ENV=LOCAL",
                    &self.container_id,
                    "/wavs/scripts/cli.sh",
                    "deploy",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait()
                .await?;

            if !res.success() {
                bail!("Failed to deploy POA middleware");
            }

            loop {
                let output = fs::read_to_string(self.nodes_dir.path().join("poa_deploy.json"))
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read POA deployment JSON: {}", e));
                if output.is_ok() {
                    break output;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await??;

        #[derive(Deserialize)]
        struct PoaDeploymentJson {
            addresses: PoaAddresses,
        }

        #[derive(Deserialize)]
        struct PoaAddresses {
            #[serde(rename = "POAStakeRegistry")]
            poa_stake_registry: Address,
            #[serde(rename = "proxyAdmin")]
            proxy_admin: Address,
        }

        let deployment_json: PoaDeploymentJson = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse POA deployment JSON: {}", e))?;

        let poa_address = deployment_json.addresses.poa_stake_registry;

        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    &self.container_id,
                    "cast",
                    "send",
                    &format!("{}", poa_address),
                    "transferOwnership(address)",
                    ANVIL_DEPLOYER_ADDRESS,
                    "--private-key",
                    &deployer_key_hex,
                    "--rpc-url",
                    &rpc_url,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !res.success() {
            bail!("Failed to transfer POA ownership to ANVIL deployer");
        }

        Ok(MiddlewareServiceManager {
            deployer_key_hex: ANVIL_DEPLOYER_KEY.to_string(),
            rpc_url,
            id,
            address: poa_address,
            proxy_admin: deployment_json.addresses.proxy_admin,
            impl_address: poa_address,
            stake_registry_address: poa_address,
            stake_registry_impl_address: poa_address,
        })
    }

    pub async fn configure_service_manager(
        &self,
        service_manager: &MiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        for i in 0..config.operators.len() {
            let operator = &config.operators[i];
            let weight = &config.weights[i];
            let avs_operator = &config.avs_operators[i];
            let res = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("docker")
                    .args([
                        "exec",
                        "-e",
                        &format!("FUNDED_KEY={}", ANVIL_DEPLOYER_KEY),
                        "-e",
                        &format!("RPC_URL={}", service_manager.rpc_url),
                        "-e",
                        "DEPLOY_ENV=LOCAL",
                        "-e",
                        &format!("POA_STAKER_REGISTRY_ADDRESS={}", service_manager.address),
                        &self.container_id,
                        "/wavs/scripts/cli.sh",
                        "owner_operation",
                        "registerOperator",
                        &format!("{:?}", operator),
                        &weight.to_string(),
                    ])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
                    .wait(),
            )
            .await??;

            if !res.success() {
                bail!("Failed to register operator");
            }

            // set signing key for each operator
            let operator_key = avs_operator.operator_private_key.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Operator private key required for POA middleware")
            })?;
            let signing_key = avs_operator
                .signer_private_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Signer private key required for POA middleware"))?;

            let res = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("docker")
                    .args([
                        "exec",
                        "-e",
                        &format!("OPERATOR_KEY={}", operator_key),
                        "-e",
                        &format!("SIGNING_KEY={}", signing_key),
                        "-e",
                        &format!("RPC_URL={}", service_manager.rpc_url),
                        "-e",
                        "DEPLOY_ENV=LOCAL",
                        "-e",
                        &format!("POA_STAKER_REGISTRY_ADDRESS={}", service_manager.address),
                        &self.container_id,
                        "/wavs/scripts/cli.sh",
                        "update_signing_key",
                    ])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
                    .wait(),
            )
            .await??;

            if !res.success() {
                bail!("Failed to update signing key");
            }
        }

        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("FUNDED_KEY={}", ANVIL_DEPLOYER_KEY),
                    "-e",
                    &format!("RPC_URL={}", service_manager.rpc_url),
                    "-e",
                    "DEPLOY_ENV=LOCAL",
                    "-e",
                    &format!("POA_STAKER_REGISTRY_ADDRESS={}", service_manager.address),
                    &self.container_id,
                    "/wavs/scripts/cli.sh",
                    "owner_operation",
                    "updateQuorum",
                    &config.quorum_numerator.to_string(),
                    &config.quorum_denominator.to_string(),
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !res.success() {
            bail!("Failed to update quorum");
        }

        Ok(())
    }

    pub async fn set_service_manager_uri(
        &self,
        service_manager: &MiddlewareServiceManager,
        service_uri: &str,
    ) -> Result<()> {
        tracing::debug!(
            "Setting service URI for POA: address={}, uri='{}'",
            service_manager.address,
            service_uri
        );
        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    &self.container_id,
                    "cast",
                    "send",
                    &format!("{}", service_manager.address),
                    "setServiceURI(string)",
                    service_uri,
                    "--private-key",
                    ANVIL_DEPLOYER_KEY,
                    "--rpc-url",
                    &service_manager.rpc_url,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !res.success() {
            bail!(
                "Failed to set service URI for address {}",
                service_manager.address
            );
        }

        Ok(())
    }
}

impl Drop for PoaMiddleware {
    fn drop(&mut self) {
        tracing::warn!("Stopping POA middleware container: {}", self.container_id);
        if let Err(e) = std::process::Command::new("docker")
            .args(["rm", "-f", &self.container_id])
            .spawn()
            .and_then(|mut cmd| cmd.wait())
        {
            tracing::warn!("Failed to remove POA middleware container: {:?}", e);
        }
    }
}
