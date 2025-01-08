// code needed for interop with the aggregator

use alloy::primitives::{Address, TxHash};
use serde::{Deserialize, Serialize};

use crate::{layer_contract_client::SignedPayload, ServiceID};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AggregateAvsRequest {
    // left as an enum to allow for future expansion
    EigenContract {
        signed_payload: SignedPayload,
        service_manager_address: Address,
    },
}

impl AggregateAvsRequest {
    pub fn signed_payload(&self) -> &SignedPayload {
        match self {
            AggregateAvsRequest::EigenContract { signed_payload, .. } => signed_payload,
        }
    }

    pub fn service_manager_address(&self) -> Address {
        match self {
            AggregateAvsRequest::EigenContract {
                service_manager_address,
                ..
            } => *service_manager_address,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AggregateAvsResponse {
    Sent { tx_hash: TxHash, count: usize },
    Aggregated { count: usize },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AddAggregatorServiceRequest {
    EthTrigger {
        /// Address of the avs
        service_manager_address: Address,
    },
}
