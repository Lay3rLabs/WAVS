use alloy::primitives::Address;
use alloy::rpc::types::TransactionReceipt;

use crate::eth_client::EthQueryClient;
use crate::eth_client::EthSigningClient;

use super::IWavsServiceManagerQueryT;

#[derive(Clone)]
pub struct ServiceManagerQueryClient {
    pub eth: EthQueryClient,
    pub address: Address,
    pub contract: IWavsServiceManagerQueryT,
}

impl ServiceManagerQueryClient {
    pub fn new(eth: EthQueryClient, address: Address) -> Self {
        let contract = IWavsServiceManagerQueryT::new(address, eth.provider.clone());

        Self {
            eth,
            address,
            contract,
        }
    }

    pub async fn get_service_metadata_uri(&self) -> anyhow::Result<String> {
        let service_url = self.contract.getWavsMetadataURI().call().await?._0;

        Ok(service_url)
    }
}

use super::IWavsServiceManagerSigningT;

#[derive(Clone)]
pub struct ServiceManagerSigningClient {
    pub eth: EthSigningClient,
    pub address: Address,
    pub contract: IWavsServiceManagerSigningT,
}

impl ServiceManagerSigningClient {
    pub fn new(eth: EthSigningClient, address: Address) -> Self {
        let contract = IWavsServiceManagerSigningT::new(address, eth.provider.clone());

        Self {
            eth,
            address,
            contract,
        }
    }

    pub async fn set_metadata_uri(
        &self,
        metadata_uri: impl ToString,
    ) -> anyhow::Result<TransactionReceipt> {
        let tx = self
            .contract
            .setWavsMetadataURI(metadata_uri.to_string())
            .send()
            .await?;

        let receipt = tx.get_receipt().await?;

        if !receipt.status() {
            return Err(anyhow::anyhow!("Transaction failed"));
        }

        Ok(receipt)
    }

    pub async fn get_service_metadata_uri(&self) -> anyhow::Result<String> {
        let service_url = self.contract.getWavsMetadataURI().call().await?._0;

        Ok(service_url)
    }
}
