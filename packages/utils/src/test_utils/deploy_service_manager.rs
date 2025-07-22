use std::process::{Command, Stdio};

use alloy_provider::DynProvider;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use wavs_types::IWavsServiceManager::{self, IWavsServiceManagerInstance};

use crate::test_utils::address::rand_address_evm;

pub const MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/wavs-middleware:0.5.0-beta.5";

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceManagerConfig {
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

impl ServiceManagerConfig {
    pub fn with_signers(signers:&[LocalSigner<SigningKey>], required_to_pass: u64) -> Self {
        let mut operators = Vec::with_capacity(signers.len() as usize);
        let mut weights = Vec::with_capacity(signers.len() as usize);

        for signer in signers.iter() { 
            operators.push(signer.address());
            weights.push(10000); // Default weight
        }

        Self {
            signing_key_addresses: operators.clone(), // Use the same addresses for signing keys
            operators,
            quorum_denominator: signers.len() as u64,
            quorum_numerator: required_to_pass,
            threshold: 1,
            weights,
        }
    }
}

impl Default for ServiceManagerConfig {
    fn default() -> Self {
        Self {
            operators: vec![
                "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
                    .parse()
                    .unwrap(),
                "0x2B5AD5c4795c026514f8317c7a215E218DcCD6cF"
                    .parse()
                    .unwrap(),
                "0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69"
                    .parse()
                    .unwrap(),
                "0x1efF47bc3a10a45D4B230B5d10E37751FE6AA718"
                    .parse()
                    .unwrap(),
                "0xe1AB8145F7E55DC933d51a18c793F901A3A0b276"
                    .parse()
                    .unwrap(),
            ],
            quorum_denominator: 3,
            quorum_numerator: 2,
            signing_key_addresses: vec![
                "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
                    .parse()
                    .unwrap(),
                "0x2B5AD5c4795c026514f8317c7a215E218DcCD6cF"
                    .parse()
                    .unwrap(),
                "0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69"
                    .parse()
                    .unwrap(),
                "0x1efF47bc3a10a45D4B230B5d10E37751FE6AA718"
                    .parse()
                    .unwrap(),
                "0xe1AB8145F7E55DC933d51a18c793F901A3A0b276"
                    .parse()
                    .unwrap(),
            ],
            threshold: 12345,
            weights: vec![10000, 10000, 10000, 10000, 10000],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceManager {
    pub config: ServiceManagerConfig,
    pub address: alloy_primitives::Address,
    pub all_addresses: ServiceManagerAddresses,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceManagerAddresses {
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

impl ServiceManager {
    pub async fn deploy(
        config: ServiceManagerConfig,
        rpc_url: String,
    ) -> anyhow::Result<ServiceManager> {
        let nodes_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();
        let config_filepath = config_dir.path().join("wavs-mock-config.json");

        std::fs::write(&config_filepath, serde_json::to_string(&config).unwrap()).unwrap();

        let seed_phrase = "test test test test test test test test test test test junk".to_string();

        let deployer_key = MnemonicBuilder::<English>::default()
            .phrase(seed_phrase)
            .index(0)
            .unwrap()
            .build()
            .unwrap()
            .into_credential()
            .to_bytes();

        let deployer_key = const_hex::encode(deployer_key);

        let image = MIDDLEWARE_IMAGE.to_string();

        let args: Vec<String> = [
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
                config_filepath
                    .to_str()
                    .ok_or(anyhow::anyhow!("Failed to convert config_filepath to str"))?
            ),
            "-e",
            &format!("MOCK_DEPLOYER_KEY={deployer_key}"),
            "-e",
            &format!("MOCK_RPC_URL={rpc_url}"),
            image.as_str(),
            "-m",
            "mock",
            "deploy",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

        let res = Command::new("docker")
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;

        if !res.success() {
            panic!("Failed to deploy service manager");
        }

        let deployment_json = std::fs::read_to_string(nodes_dir.path().join("mock.json"))
            .map_err(|e| anyhow::anyhow!("Failed to read service manager JSON: {}", e))?;

        #[derive(Deserialize)]
        struct DeploymentJson {
            addresses: ServiceManagerAddresses,
        }

        let deployment_json: DeploymentJson = serde_json::from_str(&deployment_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse service manager JSON: {}", e))?;

        Ok(Self {
            config,
            address: deployment_json.addresses.address,
            all_addresses: deployment_json.addresses,
        })
    }

    pub fn instance(&self, provider: alloy_provider::DynProvider) -> IWavsServiceManagerInstance<DynProvider> {
        IWavsServiceManager::new(self.address, provider)
    }
}
