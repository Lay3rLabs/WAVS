use config::HelloWorldAddresses;

use crate::{eigen_client::CoreAVSAddresses, eth_client::EthSigningClient};

pub mod builder;
pub mod client;
pub mod config;
pub mod solidity_types;

pub struct HelloWorldClientBuilder {
    pub eth: EthSigningClient,
    pub core_avs_addrs: Option<CoreAVSAddresses>,
}

impl HelloWorldClientBuilder {
    pub fn new(eth: EthSigningClient) -> Self {
        Self {
            eth,
            core_avs_addrs: None,
        }
    }

    pub fn avs_addresses(mut self, addresses: CoreAVSAddresses) -> Self {
        self.core_avs_addrs = Some(addresses);
        self
    }
}

pub struct HelloWorldClient {
    pub eth: EthSigningClient,
    pub core: CoreAVSAddresses,
    pub hello_world: HelloWorldAddresses,
}
