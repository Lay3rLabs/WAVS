use std::ops::Deref;

use super::layer_trigger::LayerTrigger;
use super::{
    layer_service_manager::{ILayerServiceManager::Payload, LayerServiceManager},
    layer_trigger::LayerTrigger::NewTrigger,
};
use super::{LayerContractClientFull, LayerServiceManagerT, LayerTriggerT};
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
use serde::{Deserialize, Serialize};

pub struct LayerContractClientSimple {
    pub eth: EthSigningClient,
    pub trigger_contract_address: Address,
    pub service_manager_contract_address: Address,
    pub trigger_contract: LayerTriggerT,
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
        let trigger_contract =
            LayerTrigger::new(trigger_contract_address, eth.http_provider.clone());

        Self {
            eth,
            service_manager_contract_address,
            service_manager_contract,
            trigger_contract_address,
            trigger_contract,
        }
    }

    pub async fn get_signed_data(&self, trigger_id: TriggerId) -> Result<Vec<u8>> {
        let resp = self
            .service_manager_contract
            .getSignedDataByTriggerId(*trigger_id)
            .call()
            .await
            .context("Failed to get signed data")?
            ._0;

        Ok(resp.data.to_vec())
    }

    // TODO - bring all newtypes into utils
    pub async fn add_trigger(&self, service_id: String, data: Vec<u8>) -> Result<TriggerId> {
        let event: NewTrigger = self
            .trigger_contract
            .addTrigger(service_id, data.into())
            .send()
            .await?
            .get_receipt()
            .await?
            .solidity_event()
            .context("Not found new task creation event")?;

        Ok(TriggerId::new(event.triggerId))
    }

    pub async fn get_trigger(&self, trigger_id: TriggerId) -> Result<TriggerResponse> {
        let resp = self
            .trigger_contract
            .getTrigger(*trigger_id)
            .call()
            .await
            .context("Failed to get trigger")?
            ._0;

        Ok(TriggerResponse {
            trigger_id: TriggerId::new(resp.triggerId),
            service_id: resp.serviceId,
            creator: resp.creator,
            data: resp.data.to_vec(),
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
        let message_hash = eip191_hash_message(keccak256(&payload.abi_encode()));
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

// Rust-friendly API around types
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TriggerId(u64);

impl TriggerId {
    pub fn new(val: u64) -> Self {
        Self(val)
    }
}

impl Deref for TriggerId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Debug for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct TriggerResponse {
    pub trigger_id: TriggerId,
    pub service_id: String,
    pub creator: Address,
    pub data: Vec<u8>,
}
