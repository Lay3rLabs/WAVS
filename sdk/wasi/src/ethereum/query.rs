use anyhow::Result;
use layer_climb_address::Address;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

use crate::collection::HashMapLike;

pub struct EthereumQuerier {
    pub endpoint: String,
    pub reactor: Reactor,
}

impl EthereumQuerier {
    pub fn new_from_chain_name(
        chain_name: &str,
        chain_configs: &crate::bindings::lay3r::avs::layer_types::ChainConfigs,
        reactor: Reactor,
    ) -> Result<Self> {
        let chain_config = chain_configs
            .get_key(chain_name)
            .ok_or_else(|| anyhow::anyhow!("chain config not found"))?;
        match chain_config {
            crate::wit_bindings::AnyChainConfig::Eth(config) => {
                Ok(Self::new(config.http_endpoint.to_string(), reactor))
            }
            crate::wit_bindings::AnyChainConfig::Cosmos(_) => Err(anyhow::anyhow!(
                "expected ethereum chain config, got cosmos chain"
            )),
        }
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
