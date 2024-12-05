pub mod avs_deploy;
pub mod avs_operator;
pub mod solidity_types;
use std::fmt::{self, Debug, Formatter};

use alloy::primitives::{Address, FixedBytes, U256};
use serde::{Deserialize, Serialize};
use solidity_types::{misc::AVSDirectory, HttpSigningProvider};

use crate::eth_client::EthSigningClient;

use anyhow::Result;

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

impl CoreAVSAddresses {
    pub async fn calculate_operator_avs_registration_digest_hash(
        &self,
        operator: Address,
        avs: Address,
        salt: FixedBytes<32>,
        expiry: U256,
        provider: HttpSigningProvider,
    ) -> Result<FixedBytes<32>> {
        let contract_avs_directory = AVSDirectory::new(self.avs_directory, provider);

        let operator_avs_registration_digest_hash = contract_avs_directory
            .calculateOperatorAVSRegistrationDigestHash(operator, avs, salt, expiry)
            .call()
            .await?;

        let AVSDirectory::calculateOperatorAVSRegistrationDigestHashReturn { _0: avs_hash } =
            operator_avs_registration_digest_hash;

        Ok(avs_hash)
    }
}
