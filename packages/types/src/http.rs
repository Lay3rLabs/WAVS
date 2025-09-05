pub mod aggregator;
use super::Service;
use crate::{AnyChainConfig, ChainKey, ComponentDigest, ServiceDigest, ServiceId, ServiceManager};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SigningKeyResponse {
    Secp256k1 {
        /// The derivation index used to create this key from the mnemonic
        hd_index: u32,
        /// The evm-style address ("0x" prefixed hex string) derived from the key
        evm_address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct AddServiceRequest {
    pub service_manager: ServiceManager,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct GetServiceKeyRequest {
    pub service_manager: ServiceManager,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct AddChainRequest {
    pub chain: ChainKey,
    pub config: AnyChainConfig,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct DeleteServicesRequest {
    pub service_managers: Vec<ServiceManager>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ListServicesResponse {
    pub services: Vec<Service>,
    pub service_ids: Vec<ServiceId>,
    pub component_digests: Vec<ComponentDigest>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UploadComponentResponse {
    pub digest: ComponentDigest,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct SaveServiceResponse {
    pub hash: ServiceDigest,
}
