use anyhow::Result;
use layer_climb_address::Address;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

use crate::bindings::compat::EthChainConfig;

pub struct EthereumQuerier {
    pub chain_config: EthChainConfig,
    pub reactor: Reactor,
}

impl EthereumQuerier {
    pub fn new(chain_config: EthChainConfig, reactor: Reactor) -> Self {
        Self {
            chain_config,
            reactor,
        }
    }

    pub async fn contract_smart<T: DeserializeOwned>(
        &self,
        _address: &Address,
        _query: impl Serialize,
    ) -> Result<T> {
        todo!()
    }
}
