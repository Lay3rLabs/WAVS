use serde::{Deserialize, Serialize};

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, bincode::Encode, bincode::Decode,
)]
pub struct SubmitAction {
    pub chain_name: String,
    pub contract_address: Vec<u8>,
}

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, bincode::Encode, bincode::Decode,
)]
pub struct TimerAction {
    pub delay: u64,
}

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, bincode::Encode, bincode::Decode,
)]
pub enum AggregatorAction {
    Submit(SubmitAction),
    Timer(TimerAction),
}

// conversions from WIT types to our serializable types
impl From<crate::engine::AggregatorAction> for AggregatorAction {
    fn from(action: crate::engine::AggregatorAction) -> Self {
        match action {
            crate::engine::AggregatorAction::Submit(submit) => Self::Submit(SubmitAction {
                chain_name: submit.chain_name,
                contract_address: submit.contract_address.raw_bytes,
            }),
            crate::engine::AggregatorAction::Timer(timer) => {
                Self::Timer(TimerAction { delay: timer.delay })
            }
        }
    }
}

impl From<AggregatorAction> for crate::engine::AggregatorAction {
    fn from(action: AggregatorAction) -> Self {
        match action {
            AggregatorAction::Submit(submit) => Self::Submit(crate::engine::SubmitAction {
                chain_name: submit.chain_name,
                contract_address: wavs_engine::bindings::aggregator::world::wavs::types::chain::EvmAddress {
                    raw_bytes: submit.contract_address,
                },
            }),
            AggregatorAction::Timer(timer) => Self::Timer(
                wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::TimerAction {
                    delay: timer.delay,
                },
            ),
        }
    }
}
