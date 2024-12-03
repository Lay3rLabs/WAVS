use std::path::PathBuf;

use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::{
    eigen_client::{config::CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::HelloWorldClientBuilder,
    init_tracing_tests,
};

#[tokio::test]
async fn deploy_core_contracts() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
}

#[tokio::test]
async fn deploy_hello_world_avs() {
    let EigenTestInit {
        core_contracts,
        eigen_client,
        anvil,
    } = EigenTestInit::new().await;
    let hello_world_client = HelloWorldClientBuilder::new(eigen_client.eth.clone());
    let hello_world_full_client = hello_world_client
        .avs_addresses(core_contracts)
        .build()
        .await
        .unwrap();
}

#[tokio::test]
async fn register_operator() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
    // TODO
}

struct EigenTestInit {
    pub core_contracts: CoreAVSAddresses,
    pub eigen_client: EigenClient,
    #[allow(unused)]
    pub anvil: AnvilInstance,
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

        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        Self {
            core_contracts,
            eigen_client,
            anvil,
        }
    }
}
