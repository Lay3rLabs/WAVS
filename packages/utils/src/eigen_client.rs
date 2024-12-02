use crate::eth_client::EthSigningClient;
use eigen_utils::delegationmanager::DelegationManager;

#[derive(Clone)]
pub struct EigenClient {
    pub eth_client: EthSigningClient
}

pub struct EigenClientConfig {
}

impl EigenClient {
    pub fn new(eth_client: EthSigningClient) -> Self {
        let delegation_manager = DelegationManager::new(address, eth_client.provider.clone());
        Self {
            eth_client
        }
    }
}