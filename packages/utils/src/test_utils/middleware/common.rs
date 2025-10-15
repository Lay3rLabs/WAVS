use std::ops::Deref;
use std::sync::Arc;

use alloy_primitives::Address;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

pub use super::middleware_eigen::EigenlayerMiddleware;
pub use super::middleware_poa::PoaMiddleware;

pub const MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/wavs-middleware:0.5.0-beta.10";
pub const POA_MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/poa-middleware:1.0.1";
pub const ANVIL_DEPLOYER_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
pub const ANVIL_DEPLOYER_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MiddlewareType {
    #[default]
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
    format!("mock-config-{}", id)
}

pub fn middleware_deploy_filename(id: &str) -> String {
    format!("mock-deploy-{}", id)
}

pub enum MiddlewareInstanceInner {
    Eigenlayer(EigenlayerMiddleware),
    Poa(PoaMiddleware),
}

impl MiddlewareInstanceInner {
    pub async fn new(middleware_type: MiddlewareType) -> Result<Self> {
        match middleware_type {
            MiddlewareType::Eigenlayer => Ok(MiddlewareInstanceInner::Eigenlayer(
                EigenlayerMiddleware::new().await?,
            )),
            MiddlewareType::Poa => Ok(MiddlewareInstanceInner::Poa(PoaMiddleware::new().await?)),
        }
    }

    pub async fn deploy_service_manager(
        &self,
        rpc_url: String,
        deployer_key_hex: String,
    ) -> Result<MiddlewareServiceManager> {
        match self {
            MiddlewareInstanceInner::Eigenlayer(m) => {
                m.deploy_service_manager(rpc_url, deployer_key_hex).await
            }
            MiddlewareInstanceInner::Poa(m) => {
                m.deploy_service_manager(rpc_url, deployer_key_hex).await
            }
        }
    }

    pub async fn configure_service_manager(
        &self,
        service_manager: &MiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        match self {
            MiddlewareInstanceInner::Eigenlayer(m) => {
                m.configure_service_manager(service_manager, config).await
            }
            MiddlewareInstanceInner::Poa(m) => {
                m.configure_service_manager(service_manager, config).await
            }
        }
    }

    pub async fn set_service_manager_uri(
        &self,
        service_manager: &MiddlewareServiceManager,
        service_uri: &str,
    ) -> Result<()> {
        match self {
            MiddlewareInstanceInner::Eigenlayer(m) => {
                m.set_service_manager_uri(service_manager, service_uri)
                    .await
            }
            MiddlewareInstanceInner::Poa(m) => {
                m.set_service_manager_uri(service_manager, service_uri)
                    .await
            }
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
    #[serde(skip)]
    pub container_id: Option<String>,
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

impl Drop for MiddlewareServiceManager {
    fn drop(&mut self) {
        if let Some(container_id) = &self.container_id {
            tracing::debug!("Cleaning up middleware container: {}", container_id);
            if let Err(e) = std::process::Command::new("docker")
                .args(["rm", "-f", container_id])
                .spawn()
                .and_then(|mut cmd| cmd.wait())
            {
                tracing::warn!("Failed to remove middleware container: {:?}", e);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MiddlewareServiceManagerConfig {
    #[serde(skip)]
    pub avs_operators: Vec<AvsOperator>,
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
            avs_operators: operators.to_vec(),
            signing_key_addresses: operators.iter().map(|op| op.signer).collect(),
            operators: operators.iter().map(|op| op.operator).collect(),
            quorum_denominator: (operators.len() as u64).max(1), // gotta have at least one operator
            quorum_numerator: required_to_pass,
            threshold: 1,
            weights: operators.iter().map(|op| op.weight).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AvsOperator {
    pub operator: Address,
    pub signer: Address,
    pub weight: u64,
    pub operator_private_key: Option<String>,
    pub signer_private_key: Option<String>,
}

impl AvsOperator {
    pub const DEFAULT_WEIGHT: u64 = 10000;

    pub fn new(operator: Address, signer: Address) -> Self {
        Self {
            operator,
            signer,
            weight: Self::DEFAULT_WEIGHT,
            operator_private_key: None,
            signer_private_key: None,
        }
    }

    pub fn with_keys(
        operator: Address,
        signer: Address,
        operator_private_key: String,
        signer_private_key: String,
    ) -> Self {
        Self {
            operator,
            signer,
            weight: Self::DEFAULT_WEIGHT,
            operator_private_key: Some(operator_private_key),
            signer_private_key: Some(signer_private_key),
        }
    }
}

pub async fn validate_docker_container_id(container_id: &str) -> Result<()> {
    // Validate that container_id is a valid Docker container ID
    // Docker container IDs are hexadecimal strings, typically 12 or 64 characters
    if container_id.len() < 12 || container_id.len() > 64 {
        bail!(
            "Invalid container ID length: {} (expected 12-64 characters)",
            container_id.len()
        );
    }

    if !container_id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "Invalid container ID format: '{}' (must contain only hexadecimal characters)",
            container_id
        );
    }

    // Verify the container actually exists and get its state information
    let verify_output = Command::new("docker")
        .args([
            "inspect",
            container_id,
            "--format",
            "{{.State.Running}},{{.State.Status}},{{.State.ExitCode}}",
        ])
        .output()
        .await?;

    if !verify_output.status.success() {
        bail!(
            "Container verification failed: container '{}' does not exist",
            container_id
        );
    }

    let state_info = String::from_utf8(verify_output.stdout)
        .map_err(|e| anyhow::anyhow!("Failed to parse docker inspect output as UTF-8: {}", e))?;
    let state_info = state_info.trim();

    let parts: Vec<&str> = state_info.split(',').collect();
    if parts.len() != 3 {
        bail!(
            "Unexpected docker inspect output format for container '{}': '{}'",
            container_id,
            state_info
        );
    }

    let (is_running, status, exit_code) = (parts[0], parts[1], parts[2]);

    match (is_running, status) {
        ("true", "running") => {
            // Container is running - this is what we want
        }
        ("false", "exited") => {
            bail!(
                "Container '{}' has exited with code {} and is not running",
                container_id,
                exit_code
            );
        }
        ("false", status) => {
            bail!(
                "Container '{}' is not running (status: {}, exit_code: {})",
                container_id,
                status,
                exit_code
            );
        }
        (running, status) => {
            bail!(
                "Container '{}' has unexpected state (running: {}, status: {}, exit_code: {})",
                container_id,
                running,
                status,
                exit_code
            );
        }
    }

    Ok(())
}
