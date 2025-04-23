use alloy_primitives::Address;
use anyhow::Result;
use utils::evm_client::EvmSigningClient;

use super::{
    example_submit::ISimpleSubmit::SignedData,
    solidity_types::{example_submit::SimpleSubmit, SimpleSubmitT},
    trigger::TriggerId,
};

pub struct SimpleEthSubmitClient {
    pub eth: EvmSigningClient,
    pub contract_address: Address,
    pub contract: SimpleSubmitT,
}

impl SimpleEthSubmitClient {
    pub fn new(eth: EvmSigningClient, contract_address: Address) -> Self {
        let contract = SimpleSubmit::new(contract_address, eth.provider.clone());

        Self {
            eth,
            contract_address,
            contract,
        }
    }

    // will only succeed if trigger is validated
    pub async fn trigger_validated(&self, trigger_id: TriggerId) -> bool {
        self.contract
            .isValidTriggerId(*trigger_id)
            .call()
            .await
            .unwrap_or_default()
    }

    pub async fn signed_data(&self, trigger_id: TriggerId) -> Result<SignedData> {
        if !self.trigger_validated(trigger_id).await {
            return Err(anyhow::anyhow!("trigger not validated"));
        }
        self.contract
            .getSignedData(*trigger_id)
            .call()
            .await
            .map_err(|e| e.into())
    }
}
