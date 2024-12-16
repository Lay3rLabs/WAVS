use super::layer_service_manager::{ILayerServiceManager::Payload, LayerServiceManager};
use super::{LayerContractClientFull, LayerContractClientTrigger, LayerServiceManagerT, TriggerId};
use crate::{
    alloy_helpers::SolidityEventFinder, eth_client::EthSigningClient,
    layer_contract_client::layer_service_manager::LayerServiceManager::AddedSignedDataForTrigger,
};
use alloy::{
    dyn_abi::DynSolValue,
    primitives::{eip191_hash_message, keccak256, Address, U256},
    providers::Provider,
    signers::SignerSync,
    sol_types::SolValue,
};
use anyhow::{Context, Result};

#[derive(Clone)]
pub struct LayerContractClientSimple {
    pub eth: EthSigningClient,
    pub trigger: LayerContractClientTrigger,
    pub service_manager_contract_address: Address,
    pub service_manager_contract: LayerServiceManagerT,
}

impl From<LayerContractClientFull> for LayerContractClientSimple {
    fn from(full: LayerContractClientFull) -> Self {
        Self::new(full.eth, full.layer.service_manager, full.layer.trigger)
    }
}

impl LayerContractClientSimple {
    pub fn new(
        eth: EthSigningClient,
        service_manager_contract_address: Address,
        trigger_contract_address: Address,
    ) -> Self {
        let service_manager_contract =
            LayerServiceManager::new(service_manager_contract_address, eth.http_provider.clone());
        let trigger = LayerContractClientTrigger::new(eth.clone(), trigger_contract_address);

        Self {
            eth,
            trigger,
            service_manager_contract_address,
            service_manager_contract,
        }
    }

    pub async fn get_signed_data(&self, trigger_id: TriggerId) -> Result<SignedData> {
        let resp = self
            .service_manager_contract
            .getSignedDataByTriggerId(*trigger_id)
            .call()
            .await
            .context("Failed to get signed data")?
            ._0;

        Ok(SignedData {
            data: resp.data.to_vec(),
            signature: resp.signature.to_vec(),
        })
    }

    pub async fn add_signed_trigger_data(
        &self,
        trigger_id: TriggerId,
        data: Vec<u8>,
    ) -> Result<()> {
        tracing::debug!("Signing and responding to trigger {}", trigger_id);

        let (payload, signature) = self.sign_ecdsa_trigger(trigger_id, data).await?;

        let event: AddedSignedDataForTrigger = self
            .service_manager_contract
            .addSignedDataForTrigger(payload, signature.into())
            .gas(500000)
            .send()
            .await?
            .get_receipt()
            .await?
            .solidity_event()
            .context("Unable to add signed data for trigger")?;

        if event.triggerId != *trigger_id {
            anyhow::bail!("Trigger ID mismatch");
        }

        Ok(())
    }

    // all these are just helpers, but made pub for testing, debugging, etc.
    pub async fn sign_ecdsa_trigger(
        &self,
        trigger_id: TriggerId,
        data: Vec<u8>,
    ) -> Result<(Payload, Vec<u8>)> {
        let payload = Payload {
            triggerId: *trigger_id,
            data: data.into(),
        };
        let message_hash = eip191_hash_message(keccak256(payload.abi_encode()));
        let operators: Vec<DynSolValue> = vec![DynSolValue::Address(self.eth.address())];
        let signature: Vec<DynSolValue> = vec![DynSolValue::Bytes(
            self.eth.signer.sign_hash_sync(&message_hash)?.into(),
        )];

        let current_block = U256::from(self.eth.http_provider.get_block_number().await?);

        let ecdsa_signature = DynSolValue::Tuple(vec![
            DynSolValue::Array(operators),
            DynSolValue::Array(signature),
            DynSolValue::Uint(current_block, 32),
        ])
        .abi_encode_params();

        Ok((payload, ecdsa_signature))
    }
}

pub struct SignedData {
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}
