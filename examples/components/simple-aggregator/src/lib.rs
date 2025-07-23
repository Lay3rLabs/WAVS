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
        // returning hardcoded values so the component wouldn't crash in UT
        let chain_name = host::config_var("chain_name").unwrap_or_else(|| "31337".to_string());
        let contract_address_str = host::config_var("contract_address")
            .unwrap_or_else(|| "0x0000000000000000000000000000000000000000".to_string());

        let contract_address_bytes = const_hex::decode(
            contract_address_str
                .strip_prefix("0x")
                .unwrap_or(&contract_address_str),
        )
        .map_err(|e| format!("Failed to parse contract address: {e}"))?;

        if contract_address_bytes.len() != 20 {
            return Err(format!(
                "Invalid contract address length: expected 20 bytes, got {}",
                contract_address_bytes.len()
            ));
        }

        let submit_action = SubmitAction {
            chain_name,
            contract_address: EvmAddress {
                raw_bytes: contract_address_bytes,
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
