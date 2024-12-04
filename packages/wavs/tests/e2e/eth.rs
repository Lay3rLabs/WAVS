use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::{
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{HelloWorldClient, HelloWorldClientBuilder},
};
use wavs::config::Config;

pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub avs_client: HelloWorldClient,
    anvil: AnvilInstance,
}

impl EthTestApp {
    pub async fn new(_config: Config) -> Self {
        let anvil = Anvil::new().spawn();

        let config = EthClientConfig {
            ws_endpoint: anvil.ws_endpoint().to_string(),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
        };

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

        let hello_world_client = HelloWorldClientBuilder::new(eigen_client.eth.clone())
            .avs_addresses(core_contracts)
            .build()
            .await
            .unwrap();

        Self {
            eigen_client,
            avs_client: hello_world_client,
            anvil,
        }
    }
}
