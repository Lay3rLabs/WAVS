use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::{config::EthereumChainConfig, context::AppContext};

pub fn start_chains(ctx: AppContext) -> Vec<(EthereumChainConfig, AnvilInstance)> {
    let mut chains = Vec::new();

    cfg_if::cfg_if! {
        if #[cfg(feature = "ethereum")] {
            chains.push(start_chain(ctx.clone(), 0));

            cfg_if::cfg_if! {
                if #[cfg(feature = "aggregator")] {
                    chains.push(start_chain(ctx.clone(), 1));
                }
            }
        }
    }

    chains
}

fn start_chain(ctx: AppContext, index: u8) -> (EthereumChainConfig, AnvilInstance) {
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
