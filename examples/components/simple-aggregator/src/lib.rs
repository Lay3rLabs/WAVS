mod gas_oracle;
mod world;

use wavs_wasi_utils::impl_u128_conversions;
use world::{
    host,
    wavs::aggregator::aggregator::{AggregatorAction, Packet, SubmitAction},
    wavs::types::chain::{AnyTxHash, EvmAddress},
    Guest,
};

use crate::world::wavs::aggregator::aggregator::U128;

impl_u128_conversions!(U128);

struct Component;

impl Guest for Component {
    fn process_packet(_pkt: Packet) -> Result<Vec<AggregatorAction>, String> {
        let chain = host::config_var("chain").ok_or("chain config variable is required")?;
        let service_handler_str = host::config_var("service_handler")
            .ok_or("service_handler config variable is required")?;

        let address: alloy_primitives::Address = service_handler_str
            .parse()
            .map_err(|e| format!("Failed to parse service handler address: {e}"))?;

        // Get gas price from Etherscan if configured
        // will fail the entire operation if API key is configured but fetching fails
        let gas_price = gas_oracle::get_gas_price()?;

        let submit_action = SubmitAction {
            chain,
            contract_address: EvmAddress {
                raw_bytes: address.to_vec(),
            },
            gas_price: gas_price.map(|x| x.into()),
        };

        // Sanity check that we can get the event id
        if host::get_event_id().iter().all(|x| *x == 0) {
            return Err("event id is all zeros".to_string());
        }

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_timer_callback(_packet: Packet) -> Result<Vec<AggregatorAction>, String> {
        Err("Not implemented yet".to_string())
    }

    fn handle_submit_callback(
        _packet: Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<(), String> {
        match tx_result {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

export_aggregator_world!(Component);
