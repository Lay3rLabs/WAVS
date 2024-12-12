#[allow(warnings)]
mod bindings;

use alloy_rlp::{Encodable, RlpEncodable};
use bindings::{EthInput, EthOutput, Guest};

struct Component;

#[derive(RlpEncodable)]
struct EthEventEcho {
    eth_event_data: Vec<u8>,
}

impl Guest for Component {
    fn process_eth_event(request: EthInput) -> Result<EthOutput, String> {
        let mut output = Vec::new();
        EthEventEcho {
            eth_event_data: request.event.data,
        }
        .encode(&mut output);

        Ok(EthOutput { response: output })
    }
}

bindings::export!(Component with_types_in bindings);
