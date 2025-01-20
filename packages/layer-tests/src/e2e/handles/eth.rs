use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::config::EthereumChainConfig;

use crate::e2e::config::Configs;

pub struct EthereumInstance {
    _anvil: AnvilInstance,
    _chain_config: EthereumChainConfig,
}

impl EthereumInstance {
    pub fn spawn(configs: &Configs, chain_config: EthereumChainConfig) -> Self {
        tracing::info!("Starting Ethereum chain: {}", chain_config.chain_id);
        let mut anvil = Anvil::new()
            .port(
                chain_config
                    .http_endpoint
                    .as_ref()
                    .unwrap()
                    .split(':')
                    .last()
                    .unwrap()
                    .parse::<u16>()
                    .unwrap(),
            )
            .chain_id(chain_config.chain_id.parse().unwrap());

        if let Some(anvil_interval_seconds) = configs.anvil_interval_seconds {
            anvil = anvil.block_time(anvil_interval_seconds);
        }

        let anvil = anvil.spawn();

        Self {
            _anvil: anvil,
            _chain_config: chain_config,
        }
    }
}
