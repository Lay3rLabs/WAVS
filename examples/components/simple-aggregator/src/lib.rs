mod world;

use world::{
    wavs::worker::aggregator::{AggregatorAction, EvmAddress, Packet, SubmitAction, TxResult},
    Guest,
};

struct Component;

impl Guest for Component {
    fn process_packet(_pkt: Packet) -> Result<Vec<AggregatorAction>, String> {
        // submit every packet immediately
        let submit_action = SubmitAction {
            chain_name: "ethereum".to_string(),
            contract_address: EvmAddress {
                raw_bytes: vec![0u8; 20],
            },
        };

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_timer_callback(_packet: Packet) -> Result<Vec<AggregatorAction>, String> {
        Err("No timers used".to_string())
    }

    fn handle_submit_callback(_packet: Packet, tx_result: TxResult) -> Result<bool, String> {
        match tx_result {
            TxResult::Success(_) => Ok(true),
            TxResult::Error(_) => Ok(false),
        }
    }
}

export_aggregator_world!(Component);
