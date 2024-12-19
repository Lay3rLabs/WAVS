use super::layer_service_manager::{ILayerServiceManager::Payload, LayerServiceManager};
use super::{
    solidity_types, LayerContractClientFull, LayerContractClientTrigger, LayerServiceManagerT,
    TriggerId,
};
use crate::{
    alloy_helpers::SolidityEventFinder, eth_client::EthSigningClient,
    layer_contract_client::layer_service_manager::LayerServiceManager::AddedSignedPayloadForTrigger,
};
use alloy::primitives::{FixedBytes, PrimitiveSignature};
use alloy::{
    dyn_abi::DynSolValue,
    primitives::{eip191_hash_message, keccak256, Address, U256},
    providers::Provider,
    signers::SignerSync,
    sol_types::SolValue,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
            LayerServiceManager::new(service_manager_contract_address, eth.provider.clone());
        let trigger = LayerContractClientTrigger::new(eth.clone(), trigger_contract_address);

        Self {
            eth,
            trigger,
            service_manager_contract_address,
            service_manager_contract,
        }
    }

    // only succeeds if signed data landed on-chain
    pub async fn load_signed_data(&self, trigger_id: TriggerId) -> Result<Option<SignedData>> {
        let resp = self
            .service_manager_contract
            .getSignedDataByTriggerId(*trigger_id)
            .call()
            .await
            .context("Failed to get signed data")?
            ._0;

        let data = SignedData {
            data: resp.data.to_vec(),
            signature: resp.signature.to_vec(),
        };

        if data.signature.is_empty() {
            Ok(None)
        } else {
            Ok(Some(data))
        }
    }

    // helper to add a single signed payload to the contract
    pub async fn add_signed_payload(&self, signed_payload: SignedPayload) -> Result<()> {
        let trigger_id = signed_payload.trigger_id;
        tracing::debug!("Signing and responding to trigger {}", trigger_id);

        let signed_payload_abi = signed_payload.into_submission_abi();

        let event: AddedSignedPayloadForTrigger = self
            .service_manager_contract
            .addSignedPayloadForTrigger(signed_payload_abi)
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

    pub async fn sign_payload(
        &self,
        trigger_id: TriggerId,
        data: Vec<u8>,
    ) -> Result<SignedPayload> {
        let payload = Payload {
            triggerId: *trigger_id,
            data: data.into(),
        };

        let payload_hash = eip191_hash_message(keccak256(payload.abi_encode()));

        let signature = self.eth.signer.sign_hash_sync(&payload_hash)?;

        Ok(SignedPayload {
            operator: self.eth.address(),
            trigger_id,
            data: payload.data.to_vec(),
            payload_hash,
            signature,
            signed_block_height: self.eth.provider.get_block_number().await? - 1,
        })
    }
}

// A single signed payload, meant to be passed around on the rust side
// i.e. gets sent to aggregator
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedPayload {
    pub operator: Address,
    pub trigger_id: TriggerId,
    pub data: Vec<u8>,
    pub payload_hash: FixedBytes<32>,
    pub signature: PrimitiveSignature,
    pub signed_block_height: u64,
}

impl SignedPayload {
    pub fn into_submission_abi(
        self,
    ) -> solidity_types::layer_service_manager::ILayerServiceManager::SignedPayload {
        let operators: Vec<DynSolValue> = vec![self.operator.into()];
        let signature: Vec<DynSolValue> = vec![DynSolValue::Bytes(self.signature.into())];
        let signed_block_height = U256::from(self.signed_block_height);

        let signature = DynSolValue::Tuple(vec![
            DynSolValue::Array(operators),
            DynSolValue::Array(signature),
            DynSolValue::Uint(signed_block_height, 64),
        ])
        .abi_encode_params();

        solidity_types::layer_service_manager::ILayerServiceManager::SignedPayload {
            payload: Payload {
                triggerId: *self.trigger_id,
                data: self.data.into(),
            },
            signature: signature.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedData {
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}
