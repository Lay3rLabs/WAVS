use std::{collections::HashMap, sync::RwLock};

use async_trait::async_trait;
use utils::{
    avs_client::ServiceManagerQueryClient,
    config::ChainConfigs,
    eth_client::{EthClientBuilder, EthQueryClient},
};
use wavs_types::{ChainName, ServiceMetadataSource};

use crate::{apis::service::ServiceCache, config::Config};

use super::error::ServiceError;

pub struct CoreServiceCache {
    chain_configs: ChainConfigs,
    ethereum_clients: RwLock<HashMap<ChainName, EthQueryClient>>,
    http_client: reqwest::Client,
    services: RwLock<HashMap<wavs_types::ServiceMetadataSource, wavs_types::Service>>,
}

impl CoreServiceCache {
    pub fn new(config: &Config) -> Self {
        Self {
            chain_configs: config.chains.clone(),
            ethereum_clients: RwLock::new(HashMap::new()),
            http_client: reqwest::Client::new(),
            services: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ServiceCache for CoreServiceCache {
    async fn get(
        &self,
        source: &wavs_types::ServiceMetadataSource,
    ) -> Result<wavs_types::Service, ServiceError> {
        // Check if the service is already cached
        {
            let services = self.services.read().unwrap();
            if let Some(service) = services.get(source) {
                return Ok(service.clone());
            }
        }

        let (chain_name, contract_addr) = match source {
            ServiceMetadataSource::EthereumServiceManager {
                chain_name,
                contract_address,
            } => (chain_name.clone(), contract_address.clone()),
        };

        let eth_client = {
            let lock = self.ethereum_clients.read().unwrap();
            lock.get(&chain_name).cloned()
        };

        let eth_client = match eth_client {
            Some(client) => client,
            None => {
                let chain_config = self
                    .chain_configs
                    .eth
                    .get(&chain_name)
                    .ok_or(ServiceError::MissingEthereumChain(chain_name.clone()))?;
                let client = EthClientBuilder::new(chain_config.to_client_config(None, None, None))
                    .build_query()
                    .await
                    .map_err(ServiceError::Ethereum)?;

                self.ethereum_clients
                    .write()
                    .unwrap()
                    .insert(chain_name.clone(), client.clone());

                client
            }
        };

        let service_manager_client =
            ServiceManagerQueryClient::new(eth_client.clone(), contract_addr.clone());

        let service_uri = service_manager_client
            .get_service_metadata_uri()
            .await
            .map_err(ServiceError::Ethereum)?;

        let service: wavs_types::Service = self
            .http_client
            .get(service_uri)
            .send()
            .await
            .map_err(|e| ServiceError::ServiceMetadataFetchError(e.to_string()))?
            .json()
            .await
            .map_err(|e| ServiceError::ServiceMetadataFetchError(e.to_string()))?;

        // Cache the service
        {
            let mut services = self.services.write().unwrap();
            services.insert(source.clone(), service.clone());
        }

        Ok(service)
    }
}
