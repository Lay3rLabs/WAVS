use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    pub chain_name: String,
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
    pub delay: u64,
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
