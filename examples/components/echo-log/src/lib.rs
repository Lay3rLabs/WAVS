#[allow(warnings)]
mod bindings;

use alloy_rlp::{Encodable, RlpEncodable};
use bindings::{Contract, EthLog, Guest};
use layer_wasi::parse_address_eth;

struct Component;

#[derive(RlpEncodable)]
pub struct EthOutput {
    pub address: Vec<u8>,
    pub log_topics: Vec<Vec<u8>>,
    pub log_data: Vec<u8>,
}

impl Guest for Component {
    fn run(contract: Contract, log: EthLog) -> Result<Vec<u8>, String> {
        let address =
            parse_address_eth!(bindings::lay3r::avs::wavs_types::Address, contract.address);

        let mut output = Vec::new();
        EthOutput {
            address,
            log_topics: log.topics,
            log_data: log.data,
        }
        .encode(&mut output);

        Ok(output)
    }
}

bindings::export!(Component with_types_in bindings);
