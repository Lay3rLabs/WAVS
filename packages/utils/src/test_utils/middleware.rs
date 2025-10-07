use std::ops::Deref;
use std::process::Stdio;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::Address;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;

pub const MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/wavs-middleware:0.5.0-beta.10";
pub const POA_MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/poa-middleware:v1.0.1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiddlewareType {
    Eigenlayer,
    Poa,
}

#[derive(Clone)]
pub struct MiddlewareInstance {
    inner: Arc<MiddlewareInstanceInner>,
}

impl MiddlewareInstance {
    pub async fn new(middleware_type: MiddlewareType) -> Result<Self> {
        let inner = MiddlewareInstanceInner::new(middleware_type).await?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
}

impl Deref for MiddlewareInstance {
    type Target = MiddlewareInstanceInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn middleware_config_filename(id: &str) -> String {
    format!("mock-config-{}.json", id)
}

pub fn middleware_deploy_filename(id: &str) -> String {
    format!("mock-deploy-{}.json", id)
}

pub struct MiddlewareInstanceInner {
    pub container_id: String,
    nodes_dir: TempDir,
    config_dir: TempDir,
    service_manager_count: AtomicUsize,
    middleware_type: MiddlewareType,
}

impl MiddlewareInstanceInner {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60); // enough time to pull the image and do things with it

    pub async fn new(middleware_type: MiddlewareType) -> Result<Self> {
        let nodes_dir = TempDir::new()?;
        let config_dir = TempDir::new()?;

        let image = match middleware_type {
            MiddlewareType::Eigenlayer => MIDDLEWARE_IMAGE,
            MiddlewareType::Poa => POA_MIDDLEWARE_IMAGE,
        };

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
                    image,
                    "tail",
                    "-f",
                    "/dev/null",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?
                .wait_with_output(),
        )
        .await??;

        let container_id = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("Failed to read container ID: {}", e))?
            .trim()
            .to_string();

        Ok(Self {
            container_id,
            nodes_dir,
            config_dir,
            service_manager_count: AtomicUsize::new(0),
            middleware_type,
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

        match self.middleware_type {
            MiddlewareType::Eigenlayer => {
                self.deploy_eigenlayer_service_manager(rpc_url, deployer_key_hex, id)
                    .await
            }
            MiddlewareType::Poa => {
                self.deploy_poa_service_manager(rpc_url, deployer_key_hex, id)
                    .await
            }
        }
    }

    async fn deploy_eigenlayer_service_manager(
        &self,
        rpc_url: String,
        deployer_key_hex: String,
        id: String,
    ) -> Result<MiddlewareServiceManager> {
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

    async fn deploy_poa_service_manager(
        &self,
        rpc_url: String,
        deployer_key_hex: String,
        id: String,
    ) -> Result<MiddlewareServiceManager> {
        let output = tokio::time::timeout(Self::DEFAULT_TIMEOUT, async {
            let res = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("FUNDED_KEY={deployer_key_hex}"),
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

        // https://github.com/Lay3rLabs/poa-middleware/blob/095670eb3c206f0e6c8c6951f6b81e601f989b39/contracts/script/ecdsa/POAMiddlewareDeployer.s.sol#L79-L91
        #[derive(Deserialize)]
        struct PoaAddresses {
            #[serde(rename = "POAStakeRegistry")]
            poa_stake_registry: Address,
            #[serde(rename = "proxyAdmin")]
            proxy_admin: Address,
        }

        let deployment_json: PoaDeploymentJson = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Failed to parse POA deployment JSON: {}", e))?;

        Ok(MiddlewareServiceManager {
            deployer_key_hex,
            rpc_url,
            id,
            address: deployment_json.addresses.poa_stake_registry,
            proxy_admin: deployment_json.addresses.proxy_admin,
            impl_address: deployment_json.addresses.poa_stake_registry,
            stake_registry_address: deployment_json.addresses.poa_stake_registry,
            stake_registry_impl_address: deployment_json.addresses.poa_stake_registry,
        })
    }

    pub async fn configure_service_manager(
        &self,
        service_manager: &MiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        match self.middleware_type {
            MiddlewareType::Eigenlayer => {
                self.configure_eigenlayer_service_manager(service_manager, config)
                    .await
            }
            MiddlewareType::Poa => {
                self.configure_poa_service_manager(service_manager, config)
                    .await
            }
        }
    }

    async fn configure_eigenlayer_service_manager(
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
            bail!("Failed to deploy service manager");
        }

        Ok(())
    }

    async fn configure_poa_service_manager(
        &self,
        service_manager: &MiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        // At the moment poa-middleware doesnt have a batch configuration endpoint
        // https://github.com/Lay3rLabs/poa-middleware/blob/095670eb3c206f0e6c8c6951f6b81e601f989b39/scripts/ecdsa/owner_operation.sh#L41-L48
        // register each operator with weight
        for (operator, signer, weight) in config
            .operators
            .iter()
            .zip(&config.signing_key_addresses)
            .zip(&config.weights)
            .map(|((o, s), w)| (o, s, w))
        {
            let res = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("docker")
                    .args([
                        "exec",
                        "-e",
                        &format!("FUNDED_KEY={}", service_manager.deployer_key_hex),
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
            let res = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("docker")
                    .args([
                        "exec",
                        "-e",
                        &format!("OPERATOR_KEY={}", service_manager.deployer_key_hex),
                        "-e",
                        &format!("RPC_URL={}", service_manager.rpc_url),
                        "-e",
                        "DEPLOY_ENV=LOCAL",
                        "-e",
                        &format!("POA_STAKER_REGISTRY_ADDRESS={}", service_manager.address),
                        &self.container_id,
                        "/wavs/scripts/cli.sh",
                        "update_signing_key",
                        &format!("{:?}", operator),
                        &format!("{:?}", signer),
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

        // set quorum
        let res = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("FUNDED_KEY={}", service_manager.deployer_key_hex),
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

impl Drop for MiddlewareInstanceInner {
    fn drop(&mut self) {
        tracing::warn!(
            "Stopping middleware instance with container ID: {}",
            self.container_id
        );
        if let Err(e) = std::process::Command::new("docker")
            .args(["rm", "-f", &self.container_id])
            .spawn()
            .and_then(|mut cmd| cmd.wait())
        {
            tracing::warn!("Failed to remove middleware container: {:?}", e);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MiddlewareServiceManager {
    // not part of the JSON, but used for convenience in Rust
    #[serde(skip)]
    pub deployer_key_hex: String,
    // not part of the JSON, but used for convenience in Rust
    #[serde(skip)]
    pub rpc_url: String,
    #[serde(skip)]
    // not part of the JSON, but used for convenience in Rust
    pub id: String,
    #[serde(rename = "WavsServiceManager")]
    pub address: alloy_primitives::Address,
    #[serde(rename = "proxyAdmin")]
    pub proxy_admin: alloy_primitives::Address,
    #[serde(rename = "WavsServiceManagerImpl")]
    pub impl_address: alloy_primitives::Address,
    #[serde(rename = "stakeRegistry")]
    pub stake_registry_address: alloy_primitives::Address,
    #[serde(rename = "stakeRegistryImpl")]
    pub stake_registry_impl_address: alloy_primitives::Address,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MiddlewareServiceManagerConfig {
    pub operators: Vec<alloy_primitives::Address>,
    #[serde(rename = "quorumDenominator")]
    pub quorum_denominator: u64,
    #[serde(rename = "quorumNumerator")]
    pub quorum_numerator: u64,
    #[serde(rename = "signingKeyAddresses")]
    pub signing_key_addresses: Vec<alloy_primitives::Address>,
    pub threshold: u64,
    pub weights: Vec<u64>,
}

impl MiddlewareServiceManagerConfig {
    pub fn new(operators: &[AvsOperator], required_to_pass: u64) -> Self {
        Self {
            signing_key_addresses: operators.iter().map(|op| op.signer).collect(),
            operators: operators.iter().map(|op| op.operator).collect(),
            quorum_denominator: (operators.len() as u64).max(1), // gotta have at least one operator
            quorum_numerator: required_to_pass,
            threshold: 1,
            weights: operators.iter().map(|op| op.weight).collect(),
        }
    }
}

pub struct AvsOperator {
    pub operator: Address,
    pub signer: Address,
    pub weight: u64,
}

impl AvsOperator {
    pub const DEFAULT_WEIGHT: u64 = 10000;

    pub fn new(operator: Address, signer: Address) -> Self {
        Self {
            operator,
            signer,
            weight: Self::DEFAULT_WEIGHT,
        }
    }
}
