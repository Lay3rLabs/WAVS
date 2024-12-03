pub mod avs_deploy;
pub mod avs_operator;
pub mod config;
pub mod solidity_types;
use std::sync::Arc;

use crate::eth_client::EthSigningClient;
use config::CoreAVSAddresses;

#[derive(Clone)]
pub struct EigenClient {
    pub eth: EthSigningClient,
}

impl EigenClient {
    pub fn new(eth: EthSigningClient) -> Self {
        Self { eth }
    }
}
