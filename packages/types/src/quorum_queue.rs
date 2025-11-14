use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

use super::{AggregatorAction, EventId, Packet};

#[derive(
    Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode,
)]
pub struct QuorumQueueId {
    pub event_id: EventId,
    pub aggregator_action: AggregatorAction,
}

impl QuorumQueueId {
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        bincode::encode_to_vec(self, bincode::config::standard())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        Ok(bincode::borrow_decode_from_slice(bytes, bincode::config::standard())?.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QuorumQueue {
    Burned,
    Active(Vec<QueuedPacket>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct QueuedPacket {
    pub packet: Packet,
    // so we don't need to recalculate it every time
    pub signer: Address,
}
