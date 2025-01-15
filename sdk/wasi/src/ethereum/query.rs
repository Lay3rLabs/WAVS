use anyhow::Result;
use layer_climb_address::Address;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

use crate::bindings::interface::ChainConfigs;

pub struct EthereumQuerier {
    pub endpoint: String,
    pub reactor: Reactor,
}

impl EthereumQuerier {
    pub fn new_from_chain_name(
        chain_name: &str,
        chain_configs: &ChainConfigs,
        reactor: Reactor,
    ) -> Result<Self> {
        let chain_config = chain_configs
            .eth
            .iter()
            .find_map(|(key, config)| {
                if key == chain_name {
                    Some(config)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("chain config not found"))?
            .clone();

        Ok(Self::new(chain_config.http_endpoint.to_string(), reactor))
    }

    pub fn new(endpoint: String, reactor: Reactor) -> Self {
        Self { endpoint, reactor }
    }

    pub async fn contract_smart<T: DeserializeOwned>(
        &self,
        _address: &Address,
        _query: impl Serialize,
    ) -> Result<T> {
        todo!()
    }
}
