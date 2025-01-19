use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::config::EthereumChainConfig;

use crate::config::TestConfig;

pub fn start_chains(config: &TestConfig) -> Vec<(EthereumChainConfig, AnvilInstance)> {
    let mut chains = Vec::new();

    if config.matrix.eth.regular_chain_enabled() {
        chains.push(start_chain(0, false));
    }

    if config.matrix.eth.secondary_chain_enabled() {
        chains.push(start_chain(1, false));
    }

    if config.matrix.eth.aggregator_chain_enabled() {
        chains.push(start_chain(2, true));
    }

    chains
}

fn start_chain(index: usize, aggregator: bool) -> (EthereumChainConfig, AnvilInstance) {
    let port = 8545 + index as u16;
    let chain_id = 31337 + index as u64;

    let anvil = Anvil::new().port(port).chain_id(chain_id).spawn();

    (
        EthereumChainConfig {
            chain_id: chain_id.to_string(),
            http_endpoint: Some(anvil.endpoint()),
            ws_endpoint: Some(anvil.ws_endpoint()),
            aggregator_endpoint: if aggregator {
                Some("http://127.0.0.1:8001".to_string())
            } else {
                None
            },
            faucet_endpoint: None,
        },
        anvil,
    )
}
