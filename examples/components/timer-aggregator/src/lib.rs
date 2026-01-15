mod utils;
mod world;

use example_types::{KvStoreError, KvStoreResult};
use world::{
    host,
    wavs::aggregator::input::AggregatorInput,
    wavs::aggregator::output::{AggregatorAction, EvmSubmitAction, SubmitAction, TimerAction},
    wavs::types::chain::{AnyTxHash, EvmAddress},
    wavs::types::core::Duration,
    Guest,
};

use crate::world::wasi::keyvalue::store;

struct Component;

impl Guest for Component {
    fn process_input(_input: AggregatorInput) -> Result<Vec<AggregatorAction>, String> {
        let timer_delay_secs_str = host::config_var("timer_delay_secs")
            .ok_or("timer_delay_secs config variable is required")?;

        let timer_delay_secs: u64 = timer_delay_secs_str
            .parse()
            .map_err(|e| format!("Failed to parse timer_delay_secs: {e}"))?;

        let timer_action = TimerAction {
            delay: Duration {
                secs: timer_delay_secs,
            },
        };
        Ok(vec![AggregatorAction::Timer(timer_action)])
    }

    fn handle_timer_callback(input: AggregatorInput) -> Result<Vec<AggregatorAction>, String> {
        let chain = host::config_var("chain").ok_or("chain config variable is required")?;
        let service_handler_str = host::config_var("service_handler")
            .ok_or("service_handler config variable is required")?;

        let address: alloy_primitives::Address = service_handler_str
            .parse()
            .map_err(|e| format!("Failed to parse service handler address: {e}"))?;
        let submit_action = SubmitAction::Evm(EvmSubmitAction {
            chain,
            address: EvmAddress {
                raw_bytes: address.to_vec(),
            },
            gas_price: None,
        });

        if !utils::is_valid_tx(input.trigger_action.data)? {
            return Ok(vec![]);
        }

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_submit_callback(
        _input: AggregatorInput,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<(), String> {
        write_kv_value("submit-result", "completed", "true".as_bytes())
            .map_err(|e| e.to_string())?;
        write_kv_value(
            "submit-result",
            "success",
            if tx_result.is_ok() {
                "true".as_bytes()
            } else {
                "false".as_bytes()
            },
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}

fn write_kv_value(bucket_id: &str, key: &str, value: &[u8]) -> KvStoreResult<()> {
    let bucket = open_kv_bucket(bucket_id)?;
    bucket.set(key, value).map_err(|e| KvStoreError::WriteKey {
        bucket: bucket_id.to_string(),
        key: key.to_string(),
        reason: e.to_string(),
    })
}

fn open_kv_bucket(id: &str) -> KvStoreResult<store::Bucket> {
    store::open(id).map_err(|e| KvStoreError::BucketOpen {
        id: id.to_string(),
        reason: e.to_string(),
    })
}

export_aggregator_world!(Component);
