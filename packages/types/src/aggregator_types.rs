use layer_climb_address::{CosmosAddr, EvmAddr};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ChainKey, Duration, EventId, Submission};

#[derive(
    Hash, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, bincode::Encode, bincode::Decode,
)]
pub struct QuorumQueueId {
    pub event_id: EventId,
    pub action: SubmitAction,
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
    /// Queue that has been processed and submitted on-chain
    /// Includes timestamp (seconds since UNIX epoch) when it was burned
    Burned(u64),
    Active(Vec<Submission>),
}

impl Default for QuorumQueue {
    fn default() -> Self {
        QuorumQueue::Active(Vec::new())
    }
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub enum SubmitAction {
    Evm(EvmSubmitAction),
    Cosmos(CosmosSubmitAction),
}

impl SubmitAction {
    pub fn chain(&self) -> &ChainKey {
        match self {
            SubmitAction::Evm(action) => &action.chain,
            SubmitAction::Cosmos(action) => &action.chain,
        }
    }
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub struct EvmSubmitAction {
    pub chain: ChainKey,
    // using EvmAddr from climb instead of alloy::primitives::Address for bincode support
    pub address: EvmAddr,
    pub gas_price: Option<u128>,
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub struct CosmosSubmitAction {
    pub chain: ChainKey,
    pub address: CosmosAddr,
    pub gas_price: Option<u128>,
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub struct TimerAction {
    pub delay: Duration,
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    bincode::Encode,
    bincode::Decode,
    ToSchema,
)]
pub enum AggregatorAction {
    Submit(SubmitAction),
    Timer(TimerAction),
}
