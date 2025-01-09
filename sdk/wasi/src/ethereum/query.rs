use anyhow::Result;
use layer_climb_address::Address;
use serde::{de::DeserializeOwned, Serialize};
use wstd::runtime::Reactor;

pub struct EthereumQuerier {
    pub endpoint: String,
    pub reactor: Reactor,
}

impl EthereumQuerier {
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
