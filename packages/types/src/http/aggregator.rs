use alloy_rpc_types_eth::TransactionReceipt;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Packet, Service};

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct AddPacketRequest {
    pub packet: Packet,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
pub enum AddPacketResponse {
    Sent {
        #[schema(value_type = Object)]
        tx_receipt: Box<TransactionReceipt>,
        count: usize,
    },
    Burned,
    Aggregated {
        count: usize,
    },
    Error {
        reason: String,
    },
}

// Only the operator can call this endoint
// TODO: AUTH - one idea: separate port that only allows from localhost?
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct RegisterServiceRequest {
    pub service: Service,
}
