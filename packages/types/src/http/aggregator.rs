use alloy_rpc_types_eth::TransactionReceipt;
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
    Sent {
        #[schema(value_type = Object)]
        tx_receipt: AnyTransactionReceipt,
        count: usize,
    },
    Burned,
    Aggregated {
        count: usize,
    },
    TimerStarted {
        delay_seconds: u64,
    },
    Error {
        reason: String,
    },
}

// TODO: AUTH
#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct RegisterServiceRequest {
    pub service_manager: ServiceManager,
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnyTransactionReceipt {
    #[schema(value_type = Object)]
    Evm(Box<TransactionReceipt>),
    // tx hash
    Cosmos(String),
}

impl AnyTransactionReceipt {
    pub fn tx_hash(&self) -> String {
        match self {
            AnyTransactionReceipt::Evm(receipt) => format!("{}", receipt.transaction_hash),
            AnyTransactionReceipt::Cosmos(tx_hash) => tx_hash.clone(),
        }
    }
}
