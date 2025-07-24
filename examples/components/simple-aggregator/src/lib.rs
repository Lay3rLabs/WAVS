mod world;

use world::{
    host,
    wavs::aggregator::aggregator::{AggregatorAction, Packet, SubmitAction},
    wavs::types::chain::{AnyTxHash, EvmAddress},
    Guest,
};

struct Component;

impl Guest for Component {
    fn process_packet(_pkt: Packet) -> Result<Vec<AggregatorAction>, String> {
        let chain_name =
            host::config_var("chain_name").ok_or("chain_name config variable is required")?;
        let contract_address_str = host::config_var("contract_address")
            .ok_or("contract_address config variable is required")?;

        let address: alloy_primitives::Address = contract_address_str
            .parse()
            .map_err(|e| format!("Failed to parse contract address: {e}"))?;

        let submit_action = SubmitAction {
            chain_name,
            contract_address: EvmAddress {
                raw_bytes: address.to_vec(),
            },
        };

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_timer_callback(_packet: Packet) -> Result<Vec<AggregatorAction>, String> {
        Err("No timers used".to_string())
    }

    fn handle_submit_callback(
        _packet: Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<bool, String> {
        match tx_result {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

export_aggregator_world!(Component);
