#![allow(dead_code)]
use std::{
    io::Write,
    process::Stdio,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use anyhow::Result;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::process::Command;
use wavs_types::{ChainConfigs, ChainKeyNamespace, CosmosChainConfig, CosmosChainConfigBuilder};

use crate::test_utils::middleware::operator::AvsOperator;

#[derive(Clone)]
pub struct CosmosMiddleware {
    // behind a tokio mutex so we are only ever doing one operation at a time
    inner: Arc<tokio::sync::Mutex<CosmosMiddlewareInner>>,
}
#[derive(Clone)]
pub struct CosmosMiddlewareInner {
    pub kind: CosmosMiddlewareKind,
    config_dir: Arc<TempDir>,
    env_dir: Arc<TempDir>,
    service_manager_code_id: Arc<AtomicU64>,
    signing_client: SigningClient,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CosmosMiddlewareKind {
    Mirror,
    Mock,
}

impl CosmosMiddlewareKind {
    pub fn as_str(&self) -> &str {
        match self {
            CosmosMiddlewareKind::Mirror => "mirror",
            CosmosMiddlewareKind::Mock => "mock",
        }
    }
}

impl CosmosMiddleware {
    pub async fn new(
        chain_config: CosmosChainConfig,
        kind: CosmosMiddlewareKind,
        mnemonic: String,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(tokio::sync::Mutex::new(
                CosmosMiddlewareInner::new(chain_config, kind, mnemonic).await?,
            )),
        })
    }

    pub async fn deploy_service_manager(&self) -> Result<CosmosServiceManager> {
        let addr = {
            let inner = self.inner.lock().await;
            inner.deploy_service_manager().await?
        };

        Ok(CosmosServiceManager {
            address: addr,
            middleware: self.inner.clone(),
        })
    }
}

impl CosmosMiddlewareInner {
    const DOCKER_IMAGE: &str = "ghcr.io/lay3rlabs/cw-middleware:0.2.0-alpha.4";
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub async fn new(
        chain_config: CosmosChainConfig,
        kind: CosmosMiddlewareKind,
        mnemonic: String,
    ) -> Result<Self> {
        // Write a pseudo wavs.toml file to a temp dir for our network
        let mut chain_configs = ChainConfigs::default();

        let chain_config_clone = chain_config.clone();
        chain_configs.cosmos.insert(
            chain_config_clone.chain_id.to_string().parse()?,
            CosmosChainConfigBuilder {
                bech32_prefix: chain_config_clone.bech32_prefix,
                rpc_endpoint: chain_config_clone.rpc_endpoint,
                grpc_endpoint: chain_config_clone.grpc_endpoint,
                gas_price: chain_config_clone.gas_price,
                gas_denom: chain_config_clone.gas_denom,
                faucet_endpoint: None,
            },
        );

        #[derive(Debug, Serialize, Deserialize)]
        struct PseudoConfig {
            pub default: PseudoConfigDefault,
        }
        #[derive(Debug, Serialize, Deserialize)]
        struct PseudoConfigDefault {
            pub chains: ChainConfigs,
        }

        let config = PseudoConfig {
            default: PseudoConfigDefault {
                chains: chain_configs,
            },
        };

        let config_dir = TempDir::new()?;
        let config_path = config_dir.path().join("wavs.toml");
        std::fs::write(&config_path, toml::to_string(&config)?)?;

        let env_dir = TempDir::new()?;
        let env_file_path = env_dir.path().join(".env");
        let mut env_file = std::fs::File::create(&env_file_path)?;
        writeln!(
            env_file,
            "CHAIN_KEY={}",
            config
                .default
                .chains
                .chain_keys(ChainKeyNamespace::COSMOS.parse()?)[0]
        )?;
        writeln!(env_file, "WAVS_HOME=/wavs-home")?;
        writeln!(env_file, "CLI_MNEMONIC={}", mnemonic)?;

        let key_signer = KeySigner::new_mnemonic_str(&mnemonic, None)?;

        let signing_client =
            SigningClient::new(chain_config.to_chain_config(), key_signer, None).await?;

        Ok(Self {
            kind,
            config_dir: Arc::new(config_dir),
            env_dir: Arc::new(env_dir),
            service_manager_code_id: Arc::new(AtomicU64::new(0)),
            signing_client,
        })
    }

    fn env_path(&self) -> String {
        self.env_dir
            .path()
            .join(".env")
            .to_string_lossy()
            .to_string()
    }

    pub async fn deploy_service_manager(&self) -> Result<CosmosAddr> {
        self.ensure_service_manager_uploaded().await?;

        let code_id = self
            .service_manager_code_id
            .load(std::sync::atomic::Ordering::SeqCst);

        match self.kind {
            CosmosMiddlewareKind::Mock => self.instantiate_mock_service_manager(code_id).await,
            CosmosMiddlewareKind::Mirror => {
                todo!()
            }
        }
    }

    pub async fn set_service_manager_uri(
        &self,
        servide_manager_addr: &CosmosAddr,
        uri: &str,
    ) -> Result<()> {
        let output = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "--network",
                    "host",
                    "--env-file",
                    self.env_path().as_str(),
                    "-v",
                    &format!("{}:/wavs-home", self.config_dir.path().display()),
                    Self::DOCKER_IMAGE,
                    "service-manager",
                    "set-service-uri",
                    "--address",
                    servide_manager_addr.to_string().as_str(),
                    "--uri",
                    uri,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
                .wait_with_output(),
        )
        .await??;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to set service uri service manager: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub async fn register_operator(
        &self,
        service_manager_addr: CosmosAddr,
        operator: AvsOperator,
    ) -> Result<()> {
        match self.kind {
            CosmosMiddlewareKind::Mock => {
                self.signing_client
                    .contract_execute(
                        &service_manager_addr.into(),
                        &cw_wavs_mock_api::service_manager::ExecuteMsg::SetSigningKey {
                            operator: operator.operator.into(),
                            signing_key: operator.signer.into(),
                            weight: operator.weight.into(),
                        },
                        vec![],
                        None,
                    )
                    .await?;
                Ok(())
            }
            CosmosMiddlewareKind::Mirror => {
                todo!()
            }
        }
    }

    async fn instantiate_mock_service_manager(&self, code_id: u64) -> Result<CosmosAddr> {
        let output_dir = TempDir::new()?;

        let output = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "--network",
                    "host",
                    "--env-file",
                    self.env_path().as_str(),
                    "-v",
                    &format!("{}:/wavs-home", self.config_dir.path().display()),
                    "-v",
                    &format!("{}:/output", output_dir.path().display()),
                    Self::DOCKER_IMAGE,
                    "service-manager",
                    "instantiate-mock",
                    "--code-id",
                    &code_id.to_string(),
                    "--output-path",
                    "/output/output.json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
                .wait_with_output(),
        )
        .await??;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to deploy service manager: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // read the service manager address from the output file
        let output_file_path = output_dir.path().join("output.json");
        let output_file_content = std::fs::read_to_string(output_file_path)?;

        #[derive(Debug, Serialize, Deserialize)]
        struct ServiceManagerInstantiate {
            contract_kind: String,
            address: Address,
            tx_hash: String,
        }

        let upload_info: ServiceManagerInstantiate = serde_json::from_str(&output_file_content)?;

        Ok(upload_info.address.try_into()?)
    }
    async fn ensure_service_manager_uploaded(&self) -> Result<()> {
        if self
            .service_manager_code_id
            .load(std::sync::atomic::Ordering::SeqCst)
            != 0
        {
            return Ok(());
        }

        let output_dir = TempDir::new()?;

        let output = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "--network",
                    "host",
                    "--env-file",
                    self.env_path().as_str(),
                    "-v",
                    &format!("{}:/wavs-home", self.config_dir.path().display()),
                    "-v",
                    &format!("{}:/output", output_dir.path().display()),
                    Self::DOCKER_IMAGE,
                    "service-manager",
                    "upload",
                    "--contract-kind",
                    self.kind.as_str(),
                    "--output-path",
                    "/output/output.json",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
                .wait_with_output(),
        )
        .await??;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to deploy service manager: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // read the service manager address from the output file
        let output_file_path = output_dir.path().join("output.json");
        let output_file_content = std::fs::read_to_string(output_file_path)?;

        #[derive(Debug, Serialize, Deserialize)]
        struct ServiceManagerUpload {
            contract_kind: String,
            code_id: u64,
            tx_hash: String,
        }

        let upload_info: ServiceManagerUpload = serde_json::from_str(&output_file_content)?;

        self.service_manager_code_id
            .store(upload_info.code_id, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }
}

#[derive(Clone)]
pub struct CosmosServiceManager {
    pub address: CosmosAddr,
    // behind a tokio mutex so we are only ever doing one operation at a time
    middleware: Arc<tokio::sync::Mutex<CosmosMiddlewareInner>>,
}

impl CosmosServiceManager {
    // intentionally thin, idea is to guard the lock
    pub async fn set_service_uri(&self, uri: &str) -> Result<()> {
        let inner = self.middleware.lock().await;
        inner.set_service_manager_uri(&self.address, uri).await
    }

    // intentionally thin, idea is to guard the lock
    pub async fn register_operator(&self, operator: AvsOperator) -> Result<()> {
        let inner = self.middleware.lock().await;
        inner
            .register_operator(self.address.clone(), operator)
            .await
    }
}

#[cfg(test)]
mod tests {
    use bip39::Mnemonic;
    use layer_climb::{prelude::TxSigner, querier::QueryClient};
    use layer_climb_cli::handle::CosmosInstance;
    use rand::prelude::*;
    use wavs_types::{
        contracts::cosmwasm::service_manager::ServiceManagerQueryMessages, CosmosChainConfig,
        CosmosChainConfigBuilder,
    };

    use crate::init_tracing_tests;

    use super::*;

    #[tokio::test]
    async fn cosmos_middleware_works() {
        init_tracing_tests();

        let mut rng = rand::rng();

        let entropy: [u8; 32] = rng.random();
        let mnemonic = Mnemonic::from_entropy(&entropy).unwrap().to_string();

        let (_instance, chain_config) = start_chain(&mnemonic).await;

        let middleware =
            CosmosMiddleware::new(chain_config.clone(), CosmosMiddlewareKind::Mock, mnemonic)
                .await
                .unwrap();

        let service_manager = middleware.deploy_service_manager().await.unwrap();

        service_manager
            .set_service_uri("http://example.com")
            .await
            .unwrap();

        let cosmos_client = QueryClient::new(chain_config.into(), None).await.unwrap();

        let uri: String = cosmos_client
            .contract_smart(
                &service_manager.address.clone().into(),
                &ServiceManagerQueryMessages::WavsServiceUri {},
            )
            .await
            .unwrap();

        assert_eq!(uri, "http://example.com");
    }

    async fn start_chain(mnemonic: &str) -> (CosmosInstance, CosmosChainConfig) {
        let cosmos_port = 9321;
        let rpc_endpoint = format!("http://127.0.0.1:{}", cosmos_port);

        let chain_config = CosmosChainConfigBuilder {
            rpc_endpoint: Some(rpc_endpoint),
            grpc_endpoint: None,
            gas_price: 0.025,
            gas_denom: "ucosm".to_string(),
            bech32_prefix: "wasm".to_string(),
            faucet_endpoint: None,
        }
        .build("wasmd".parse().unwrap());

        let climb_chain_config: layer_climb::prelude::ChainConfig =
            chain_config.clone().to_chain_config();

        let signer = layer_climb::prelude::KeySigner::new_mnemonic_str(mnemonic, None).unwrap();

        let addr = climb_chain_config
            .address_from_pub_key(&signer.public_key().await.unwrap())
            .unwrap();

        let instance = layer_climb_cli::handle::CosmosInstance::new(climb_chain_config, vec![addr]);

        tracing::info!(
            "Setting up Cosmos chain: {}",
            instance.chain_config.chain_id
        );
        instance.setup().unwrap();

        tracing::info!("Starting Cosmos chain: {}", instance.chain_config.chain_id);
        instance.run().unwrap();

        tracing::info!(
            "Waiting for block on Cosmos chain: {}",
            instance.chain_config.chain_id
        );
        instance.wait_for_block().await.unwrap();

        (instance, chain_config)
    }
}
