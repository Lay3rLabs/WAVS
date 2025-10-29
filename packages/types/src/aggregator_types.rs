use layer_climb_address::{CosmosAddr, EvmAddr};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ChainKey, Duration};

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
pub enum SubmitAction {
    Evm(EvmSubmitAction),
    Cosmos(CosmosSubmitAction),
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
