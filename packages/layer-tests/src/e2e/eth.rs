use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::config::EthereumChainConfig;

use crate::config::TestConfig;

pub fn start_chains(config: &TestConfig) -> Vec<(EthereumChainConfig, AnvilInstance)> {
    let mut chains = Vec::new();

    for index in 0..config.matrix.eth_chain_count() {
        chains.push(start_chain(index));
    }

    chains
}

fn start_chain(index: usize) -> (EthereumChainConfig, AnvilInstance) {
    let port = 8545 + index as u16;
    let chain_id = 31337 + index as u64;

    let anvil = Anvil::new().port(port).chain_id(chain_id).spawn();

    (
        EthereumChainConfig {
            chain_id: chain_id.to_string(),
            http_endpoint: anvil.endpoint(),
            ws_endpoint: anvil.ws_endpoint(),
            aggregator_endpoint: None,
            faucet_endpoint: None,
        },
        anvil,
    )
}
