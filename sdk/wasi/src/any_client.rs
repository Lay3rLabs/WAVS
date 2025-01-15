use crate::bindings::interface::ChainConfigs;
use crate::cosmos::CosmosQuerier;
use crate::ethereum::EthereumQuerier;
use anyhow::{anyhow, Result};
use wstd::runtime::Reactor;

pub enum AnyClient {
    Eth(EthereumQuerier),
    Cosmos(CosmosQuerier),
}

impl AnyClient {
    pub fn new_from_chain_name(
        chain_name: &str,
        chain_configs: &ChainConfigs,
        reactor: Reactor,
    ) -> Result<Self> {
        let eth_chain_config =
            chain_configs
                .eth
                .iter()
                .find_map(|(k, v)| if *k == chain_name { Some(v) } else { None });

        let cosmos_chain_config =
            chain_configs
                .cosmos
                .iter()
                .find_map(|(k, v)| if *k == chain_name { Some(v) } else { None });

        match (eth_chain_config, cosmos_chain_config) {
            (None, None) => Err(anyhow!("chain {} not found", chain_name)),
            (Some(_), Some(_)) => Err(anyhow!("chain {} found in both cosmos and eth", chain_name)),

            (Some(chain_config), None) => Ok(Self::Eth(EthereumQuerier::new(
                chain_config.http_endpoint.clone(),
                reactor,
            ))),
            (None, Some(chain_config)) => Ok(Self::Cosmos(CosmosQuerier::new(
                chain_config.clone().into(),
                reactor,
            ))),
        }
    }

    pub fn unchecked_eth(self) -> EthereumQuerier {
        match self {
            Self::Eth(eth) => eth,
            _ => panic!("expected eth client"),
        }
    }

    pub fn unchecked_cosmos(self) -> CosmosQuerier {
        match self {
            Self::Cosmos(cosmos) => cosmos,
            _ => panic!("expected cosmos client"),
        }
    }
}
