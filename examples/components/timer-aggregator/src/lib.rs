mod world;

use world::{
    host,
    wavs::aggregator::aggregator::{AggregatorAction, Packet, SubmitAction, TimerAction},
    wavs::types::chain::{AnyTxHash, EvmAddress},
    wavs::types::core::Duration,
    Guest,
};

struct Component;

impl Guest for Component {
    fn process_packet(_pkt: Packet) -> Result<Vec<AggregatorAction>, String> {
        let timer_action = TimerAction {
            delay: Duration { secs: 5 },
        };
        Ok(vec![AggregatorAction::Timer(timer_action)])
    }

    fn handle_timer_callback(_packet: Packet) -> Result<Vec<AggregatorAction>, String> {
        let chain_name =
            host::config_var("chain_name").ok_or("chain_name config variable is required")?;
        let service_handler_str = host::config_var("service_handler")
            .ok_or("service_handler config variable is required")?;

        let address: alloy_primitives::Address = service_handler_str
            .parse()
            .map_err(|e| format!("Failed to parse service handler address: {e}"))?;
        let submit_action = SubmitAction {
            chain_name,
            contract_address: EvmAddress {
                raw_bytes: address.to_vec(),
            },
        };

        Ok(vec![AggregatorAction::Submit(submit_action)])
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
