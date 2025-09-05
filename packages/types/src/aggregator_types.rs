use alloy_primitives::hex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Duration;

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
pub struct SubmitAction {
    pub chain: String,
    #[serde(with = "hex")]
    pub contract_address: Vec<u8>,
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
