pub mod aggregator;
use std::collections::HashMap;

use super::Service;
use crate::{
    AnyChainConfig, ByteArray, ChainKey, ComponentDigest, ServiceDigest, ServiceId, ServiceManager,
    Trigger, TriggerData, WorkflowId,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignerResponse {
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
pub struct GetSignerRequest {
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

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct SimulatedTriggerRequest {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub trigger: Trigger,
    #[schema(value_type = Object)]
    pub data: TriggerData,
    #[serde(default = "default_simulated_trigger_count")]
    pub count: usize,
    pub wait_for_completion: bool,
}

fn default_simulated_trigger_count() -> usize {
    1
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DevTriggerStreamsInfo {
    pub chains: HashMap<ChainKey, DevTriggerStreamInfo>,
}

impl DevTriggerStreamsInfo {
    pub fn finalized(&self) -> bool {
        self.chains.values().all(|info| {
            !info.any_active_rpcs_in_flight && info.is_connected && info.current_endpoint.is_some()
        })
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DevTriggerStreamInfo {
    pub current_endpoint: Option<String>,
    pub is_connected: bool,
    pub any_active_rpcs_in_flight: bool,
    pub active_subscriptions: HashMap<String, DevTriggerStreamSubscriptionKind>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum DevTriggerStreamSubscriptionKind {
    NewHeads,
    Logs {
        addresses: Vec<ByteArray<20>>,
        topics: Vec<ByteArray<32>>,
    },
    NewPendingTransactions,
}
