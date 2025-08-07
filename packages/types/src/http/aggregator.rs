use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Packet, ServiceManager};

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct AddPacketRequest {
    pub packet: Packet,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub enum AddPacketResponse {
    Sent { tx_hash: String, count: usize },
    Burned,
    Aggregated { count: usize },
    Error { reason: String },
}

// TODO: AUTH
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct RegisterServiceRequest {
    pub service_manager: ServiceManager,
}
