pub mod aggregator;
use super::{Service, ServiceID};
use crate::{AnyChainConfig, ChainName, ComponentDigest, ServiceDigest};
use layer_climb_address::Address;
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
    pub chain_name: ChainName,
    #[schema(value_type = Object)]
    pub address: Address,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub struct AddChainRequest {
    pub chain_name: ChainName,
    pub chain_config: AnyChainConfig,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct DeleteServicesRequest {
    pub service_ids: Vec<ServiceID>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ListServicesResponse {
    pub services: Vec<Service>,
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
