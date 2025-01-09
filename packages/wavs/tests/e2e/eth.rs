use alloy::node_bindings::AnvilInstance;
use utils::{
    avs_client::AvsClientBuilder,
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    example_client::{SimpleSubmitClient, SimpleTriggerClient},
};
use wavs::config::Config;

#[allow(dead_code)]
pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub core_contracts: CoreAVSAddresses,
    anvil: AnvilInstance,
}

impl EthTestApp {
    pub async fn new(_config: Config, anvil: AnvilInstance) -> Self {
        let config = EthClientConfig {
            ws_endpoint: Some(anvil.ws_endpoint().to_string()),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
            transport: None,
        };

        tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        eigen_client
            .register_operator(&core_contracts)
            .await
            .unwrap();

        Self {
            eigen_client,
            anvil,
            core_contracts,
        }
    }

    pub async fn deploy_service_contracts(&self) -> (SimpleTriggerClient, SimpleSubmitClient) {
        let avs_client = AvsClientBuilder::new(self.eigen_client.eth.clone())
            .core_addresses(self.core_contracts.clone())
            .build(SimpleSubmitClient::deploy)
            .await
            .unwrap();

        avs_client
            .register_operator(&mut rand::rngs::OsRng)
            .await
            .unwrap();

        let submit_client =
            SimpleSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

        let trigger_client = SimpleTriggerClient::new_deploy(avs_client.eth.clone())
            .await
            .unwrap();

        (trigger_client, submit_client)
    }
}
