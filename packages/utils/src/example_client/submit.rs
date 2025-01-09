use crate::{
    avs_client::{ServiceManagerDeps, SignedData},
    eth_client::EthSigningClient,
};
use alloy::primitives::Address;
use anyhow::Result;

use super::{
    solidity_types::{example_submit::SimpleSubmit, SimpleSubmitT},
    trigger::TriggerId,
};

#[derive(Clone)]
pub struct SimpleSubmitClient {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: SimpleSubmitT,
}

impl SimpleSubmitClient {
    pub fn new(eth: EthSigningClient, contract_address: Address) -> Self {
        let contract = SimpleSubmit::new(contract_address, eth.provider.clone());

        Self {
            eth,
            contract_address,
            contract,
        }
    }

    pub async fn deploy(deps: ServiceManagerDeps) -> Result<Address> {
        let ServiceManagerDeps {
            provider,
            avs_directory,
            stake_registry,
            rewards_coordinator,
            delegation_manager,
        } = deps;

        let contract = SimpleSubmit::deploy(
            provider,
            avs_directory,
            stake_registry,
            rewards_coordinator,
            delegation_manager,
        )
        .await?;

        Ok(*contract.address())
    }

    // will only succeed if trigger is validated
    pub async fn trigger_validated(&self, trigger_id: TriggerId) -> bool {
        self.contract
            .isValidTriggerId(*trigger_id)
            .call()
            .await
            .map(|x| x._0)
            .unwrap_or_default()
    }

    pub async fn signed_data_for_trigger(&self, trigger_id: TriggerId) -> Result<SignedData> {
        let resp = self
            .contract
            .getSignedPayloadForTriggerId(*trigger_id)
            .call()
            .await
            .map(|x| x.signedPayload)?;

        Ok(SignedData {
            data: resp.data.to_vec(),
            signature: resp.signature.to_vec(),
        })
    }
}
