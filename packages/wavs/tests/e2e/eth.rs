use alloy::node_bindings::AnvilInstance;
use utils::{
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    layer_contract_client::{LayerContractClientFull, LayerContractClientFullBuilder},
};
use wavs::config::Config;

#[allow(dead_code)]
pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub avs_client: LayerContractClientFull,
    anvil: AnvilInstance,
}

impl EthTestApp {
    pub async fn new(_config: Config, anvil: AnvilInstance) -> Self {
        let config = EthClientConfig {
            chain_id: anvil.chain_id().to_string(),
            ws_endpoint: Some(anvil.ws_endpoint().to_string()),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
            transport: None,
        };

        tracing::info!(
            "(chain_id: {}) Creating eth client on: {:?}",
            anvil.chain_id(),
            config.ws_endpoint
        );

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        eigen_client
            .register_operator(&core_contracts)
            .await
            .unwrap();

        let avs_client = LayerContractClientFullBuilder::new(eigen_client.eth.clone())
            .avs_addresses(core_contracts)
            .build()
            .await
            .unwrap();

        avs_client
            .register_operator(&mut rand::rngs::OsRng)
            .await
            .unwrap();

        Self {
            eigen_client,
            avs_client,
            anvil,
        }
    }

    pub fn chain_id(&self) -> String {
        self.anvil.chain_id().to_string()
    }
}
