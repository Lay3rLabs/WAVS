use std::path::PathBuf;

use alloy::node_bindings::Anvil;
use utils::{
    eigen_client::{avs_deploy::EigenCoreContracts, config::EigenClientConfig, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    init_tracing_tests,
};

#[tokio::test]
async fn deploy_core_contracts() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
}

#[tokio::test]
async fn deploy_hello_world_avs() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
}

#[tokio::test]
async fn register_operator() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
    // TODO
}

struct EigenTestInit {
    pub core_contracts: EigenCoreContracts,
    pub eigen_client: EigenClient,
}

impl EigenTestInit {
    pub async fn new() -> Self {
        init_tracing_tests();

        let anvil = Anvil::new().try_spawn().unwrap();

        let config = EthClientConfig {
            ws_endpoint: anvil.ws_endpoint().to_string(),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
        };

        let builder = EthClientBuilder::new(config);
        let eth_client = builder.build_signing().await.unwrap();

        let deployments_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("deployments");

        let core_deployment_data =
            tokio::fs::read_to_string(deployments_dir.join("core").join("31337.json"))
                .await
                .unwrap();

        let hello_world_deployment_data =
            tokio::fs::read_to_string(deployments_dir.join("hello-world").join("31337.json"))
                .await
                .unwrap();

        let eigen_config = EigenClientConfig {
            core: serde_json::from_str(&core_deployment_data).unwrap(),
            avs: serde_json::from_str(&hello_world_deployment_data).unwrap(),
        };

        let eigen_client = EigenClient::new(eth_client, eigen_config);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

        Self {
            core_contracts,
            eigen_client,
        }
    }
}
