#[allow(warnings)]
mod bindings;

use alloy_rlp::{Encodable, RlpEncodable};
use bindings::{EthLog, Guest};

struct Component;

#[derive(RlpEncodable)]
pub struct EthOutput {
    pub address: Vec<u8>,
    pub log_topics: Vec<Vec<u8>>,
    pub log_data: Vec<u8>,
}

impl Guest for Component {
    fn process_eth_event(log: EthLog) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        EthOutput {
            address: log.address,
            log_topics: log.log_topics,
            log_data: log.log_data,
        }
        .encode(&mut output);

        Ok(output)
    }
}

bindings::export!(Component with_types_in bindings);
