pub mod avs_deploy;
pub mod avs_operator;
pub mod config;
pub mod solidity_types;
use std::fmt::{self, Debug, Formatter};

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
