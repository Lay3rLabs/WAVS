pub mod avs_deploy;
pub mod avs_operator;
pub mod solidity_types;
use std::fmt::{self, Debug, Formatter};

use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

use crate::eth_client::EthSigningClient;

#[derive(Clone)]
pub struct EigenClient {
    pub eth: EthSigningClient,
}

impl Debug for EigenClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("EigenClient")
            .field("ws_endpoint", &self.eth.config.ws_endpoint)
            .field("http_endpoint", &self.eth.config.http_endpoint)
            .field("address", &self.eth.address())
            .finish()
    }
}

impl EigenClient {
    pub fn new(eth: EthSigningClient) -> Self {
        Self { eth }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CoreAVSAddresses {
    pub proxy_admin: Address,
    pub delegation_manager: Address,
    pub strategy_manager: Address,
    pub eigen_pod_manager: Address,
    pub eigen_pod_beacon: Address,
    pub pauser_registry: Address,
    pub strategy_factory: Address,
    pub strategy_beacon: Address,
    pub avs_directory: Address,
    pub rewards_coordinator: Address,
}
