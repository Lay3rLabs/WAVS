// Conversions between WIT types and wavs-types

use wavs_types::{AggregatorAction, Duration, SubmitAction, TimerAction};

pub fn from_engine_action(action: crate::engine::AggregatorAction) -> AggregatorAction {
    match action {
        crate::engine::AggregatorAction::Submit(submit) => AggregatorAction::Submit(SubmitAction {
            chain_name: submit.chain_name,
            contract_address: submit.contract_address.raw_bytes,
        }),
        crate::engine::AggregatorAction::Timer(timer) => AggregatorAction::Timer(TimerAction {
            delay: Duration {
                secs: timer.delay.secs,
            },
        }),
    }
}

pub fn to_engine_action(action: AggregatorAction) -> crate::engine::AggregatorAction {
    match action {
        AggregatorAction::Submit(submit) => {
            crate::engine::AggregatorAction::Submit(crate::engine::SubmitAction {
                chain_name: submit.chain_name,
                contract_address:
                    wavs_engine::bindings::aggregator::world::wavs::types::chain::EvmAddress {
                        raw_bytes: submit.contract_address,
                    },
            })
        }
        AggregatorAction::Timer(timer) => crate::engine::AggregatorAction::Timer(
            wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::TimerAction {
                delay: wavs_engine::bindings::aggregator::world::wavs::types::core::Duration {
                    secs: timer.delay.secs,
                },
            },
        ),
    }
}
