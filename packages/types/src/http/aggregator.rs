use alloy_rpc_types_eth::TransactionReceipt;
use serde::{Deserialize, Serialize};

use crate::{Packet, Service};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AddPacketRequest {
    pub packet: Packet,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AddPacketResponse {
    Sent {
        tx_receipt: Box<TransactionReceipt>,
        count: usize,
    },
    Aggregated {
        count: usize,
    },
}

// Only the operator can call this endoint
// TODO: AUTH - one idea: separate port that only allows from localhost?
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct RegisterServiceRequest {
    pub service: Service,
}
