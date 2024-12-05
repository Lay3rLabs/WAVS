use alloy::node_bindings::AnvilInstance;
use utils::{
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{HelloWorldFullClient, HelloWorldFullClientBuilder},
};
use wavs::config::Config;

#[allow(dead_code)]
pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub avs_client: HelloWorldFullClient,
    anvil: AnvilInstance,
}

impl EthTestApp {
    pub async fn new(_config: Config, anvil: AnvilInstance) -> Self {
        let config = EthClientConfig {
            ws_endpoint: anvil.ws_endpoint().to_string(),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
        };

        tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        eigen_client
            .register_operator(&core_contracts)
            .await
            .unwrap();

        let hello_world_client = HelloWorldFullClientBuilder::new(eigen_client.eth.clone())
            .avs_addresses(core_contracts)
            .build()
            .await
            .unwrap();
        hello_world_client.register_operator().await.unwrap();

        Self {
            eigen_client,
            avs_client: hello_world_client,
            anvil,
        }
    }
}
