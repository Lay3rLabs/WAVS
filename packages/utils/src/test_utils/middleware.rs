use std::process::{Command, Stdio};

use alloy_primitives::Address;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

pub const MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/wavs-middleware:0.5.0-beta.7";

#[derive(Serialize, Deserialize, Debug)]
pub struct MiddlewareServiceManagerAddresses {
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
impl MiddlewareServiceManagerAddresses {
    pub async fn deploy(rpc_url: &str, deployer_key_hex: &str) -> Result<Self> {
        let nodes_dir = TempDir::new().unwrap();

        // https://github.com/Lay3rLabs/wavs-middleware?tab=readme-ov-file#2-deploy-empty-mock-contracts
        let res = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                "host",
                "-v",
                &format!(
                    "{}:/root/.nodes",
                    nodes_dir
                        .path()
                        .to_str()
                        .ok_or(anyhow::anyhow!("Failed to convert nodes_dir path to str"))?
                ),
                "-e",
                &format!("MOCK_DEPLOYER_KEY={}", deployer_key_hex),
                "-e",
                &format!("MOCK_RPC_URL={rpc_url}"),
                MIDDLEWARE_IMAGE,
                "-m",
                "mock",
                "deploy",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;

        if !res.success() {
            bail!("Failed to deploy service manager");
        }

        let deployment_json = std::fs::read_to_string(nodes_dir.path().join("mock.json"))
            .map_err(|e| anyhow::anyhow!("Failed to read service manager JSON: {}", e))?;

        #[derive(Deserialize)]
        struct DeploymentJson {
            addresses: MiddlewareServiceManagerAddresses,
        }

        let deployment_json: DeploymentJson = serde_json::from_str(&deployment_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse service manager JSON: {}", e))?;

        Ok(deployment_json.addresses)
    }
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

    pub async fn apply(
        &self,
        rpc_url: &str,
        deployer_key_hex: &str,
        service_manager_address: &Address,
    ) -> Result<()> {
        let nodes_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();
        let config_filepath = config_dir.path().join("wavs-mock-config.json");

        std::fs::write(&config_filepath, serde_json::to_string(self).unwrap()).unwrap();

        let res = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                "host",
                "-v",
                &format!(
                    "{}:/root/.nodes",
                    nodes_dir
                        .path()
                        .to_str()
                        .ok_or(anyhow::anyhow!("Failed to convert nodes_dir path to str"))?
                ),
                "-v",
                &format!(
                    "{}:/wavs/contracts/deployments/wavs-mock-config.json",
                    config_filepath.to_str().ok_or(anyhow::anyhow!(
                        "Failed to convert config_filepath path to str"
                    ))?
                ),
                "-e",
                &format!("MOCK_DEPLOYER_KEY={}", deployer_key_hex),
                "-e",
                &format!("MOCK_RPC_URL={rpc_url}"),
                "-e",
                &format!("MOCK_SERVICE_MANAGER_ADDRESS={service_manager_address}"),
                MIDDLEWARE_IMAGE,
                "-m",
                "mock",
                "configure",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;

        if !res.success() {
            bail!("Failed to deploy service manager");
        }

        Ok(())
    }
}

pub struct MiddlewareSetServiceUri {
    pub rpc_url: String,
    pub service_manager_address: alloy_primitives::Address,
    pub deployer_key_hex: String,
    pub service_uri: String,
}

impl MiddlewareSetServiceUri {
    pub async fn apply(self) -> Result<()> {
        let Self {
            rpc_url,
            service_manager_address,
            deployer_key_hex,
            service_uri,
        } = self;
        let res = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--network",
                "host",
                "-e",
                &format!("RPC_URL={}", rpc_url),
                "-e",
                &format!("WAVS_SERVICE_MANAGER_ADDRESS={service_manager_address}"),
                "-e",
                &format!("FUNDED_KEY={deployer_key_hex}"),
                "-e",
                &format!("SERVICE_URI={service_uri}"),
                MIDDLEWARE_IMAGE,
                "set_service_uri",
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;

        if !res.success() {
            bail!("Failed to set service URI");
        }

        Ok(())
    }
}
