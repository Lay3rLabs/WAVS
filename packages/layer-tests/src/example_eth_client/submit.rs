use alloy_primitives::Address;
use alloy_sol_types::SolValue;
use anyhow::Result;
use utils::eth_client::EthSigningClient;
use wavs_types::Envelope;

use super::{
    example_submit::DataWithId,
    solidity_types::{example_submit::SimpleSubmit, SimpleSubmitT},
    trigger::TriggerId,
};

pub struct SimpleEthSubmitClient {
    pub eth: EthSigningClient,
    pub contract_address: Address,
    pub contract: SimpleSubmitT,
}

impl SimpleEthSubmitClient {
    pub fn new(eth: EthSigningClient, contract_address: Address) -> Self {
        let contract = SimpleSubmit::new(contract_address, eth.provider.clone());

        Self {
            eth,
            contract_address,
            contract,
        }
    }

    // just a static helper to simulate the data that would be sent to the contract
    pub fn data_with_id_bytes(trigger_id: u64, data: impl AsRef<[u8]>) -> Vec<u8> {
        DataWithId {
            triggerId: trigger_id,
            data: data.as_ref().to_vec().into(),
        }
        .abi_encode()
    }

    // just a static helper to help with tests
    pub fn decode_data_with_id_bytes(bytes: &[u8]) -> Result<(TriggerId, Vec<u8>)> {
        let data_with_id = DataWithId::abi_decode(bytes)?;
        Ok((
            TriggerId::new(data_with_id.triggerId),
            data_with_id.data.to_vec(),
        ))
    }

    // will only succeed if trigger is validated
    pub async fn trigger_validated(&self, trigger_id: TriggerId) -> bool {
        self.contract
            .isValidTriggerId(*trigger_id)
            .call()
            .await
            .unwrap_or_default()
    }

    pub async fn trigger_data(&self, trigger_id: TriggerId) -> Result<Vec<u8>> {
        if !self.trigger_validated(trigger_id).await {
            return Err(anyhow::anyhow!("trigger not validated"));
        }
        self.contract
            .getData(*trigger_id)
            .call()
            .await
            .map(|data| data.to_vec())
            .map_err(|e| e.into())
    }

    pub async fn trigger_envelope(&self, trigger_id: TriggerId) -> Result<Envelope> {
        if !self.trigger_validated(trigger_id).await {
            return Err(anyhow::anyhow!("trigger not validated"));
        }

        let envelope = self.contract.getEnvelope(*trigger_id).call().await?;

        Ok(Envelope {
            eventId: envelope.eventId,
            ordering: envelope.ordering,
            payload: envelope.payload,
        })
    }

    pub async fn trigger_signature(&self, trigger_id: TriggerId) -> Result<Vec<u8>> {
        if !self.trigger_validated(trigger_id).await {
            return Err(anyhow::anyhow!("trigger not validated"));
        }

        self.contract
            .getSignature(*trigger_id)
            .call()
            .await
            .map(|signature_data| signature_data.signatures[0].to_vec())
            .map_err(|e| e.into())
    }
}
