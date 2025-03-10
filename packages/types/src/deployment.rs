use serde::{Deserialize, Serialize};
use alloy_primitives::Address;

#[derive(Debug, Serialize, Deserialize)]
pub struct EigenlayerDeployment {
    #[serde(rename = "lastUpdate")]
    pub last_update: LastUpdate,
    pub addresses: EigenlayerAddresses,
    #[serde(rename = "metaDataURI")]
    pub meta_data_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastUpdate {
    pub timestamp: String,
    pub block_number: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EigenlayerAddresses {
    pub proxy_admin: Address,
    pub layer_service_manager: Address,
    pub layer_service_manager_impl: Address,
    pub stake_registry: Address,
    pub stake_registry_impl: Address,
    pub strategy: Address,
    pub token: Address,
    pub avs_registrar: Address,
    pub offchain_message_consumer: Address,
}
