pub mod avs_deploy;
pub mod avs_operator;
pub mod config;
pub mod solidity_types;
use std::sync::Arc;

use crate::eth_client::EthSigningClient;
use config::EigenClientConfig;

#[derive(Clone)]
pub struct EigenClient {
    pub eth: EthSigningClient,
    pub config: Arc<EigenClientConfig>,
}

impl EigenClient {
    pub fn new(eth: EthSigningClient, config: EigenClientConfig) -> Self {
        Self {
            eth,
            config: Arc::new(config),
        }
    }
}
