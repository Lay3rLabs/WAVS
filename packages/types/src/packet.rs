pub use crate::solidity_types::Envelope;
use crate::{ServiceID, TriggerAction, TriggerConfig, WorkflowID};
use alloy_primitives::{eip191_hash_message, keccak256, FixedBytes};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolValue;
use async_trait::async_trait;
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Packet {
    pub route: PacketRoute,
    pub envelope: Envelope,
    pub signature: EnvelopeSignature,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait EnvelopeExt {
    fn eip191_hash(&self) -> FixedBytes<32>;

    async fn sign(&self, signer: &PrivateKeySigner) -> alloy_signer::Result<EnvelopeSignature> {
        signer
            .sign_hash(&self.eip191_hash())
            .await
            .map(EnvelopeSignature::Secp256k1)
    }
}

impl EnvelopeExt for Envelope {
    fn eip191_hash(&self) -> FixedBytes<32> {
        let envelope_bytes = self.abi_encode();
        eip191_hash_message(keccak256(&envelope_bytes))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeSignature {
    Secp256k1(alloy_primitives::Signature),
}

impl EnvelopeSignature {
    pub fn eth_signer_address(
        &self,
        envelope: &Envelope,
    ) -> anyhow::Result<alloy_primitives::Address> {
        match self {
            EnvelopeSignature::Secp256k1(sig) => sig
                .recover_address_from_prehash(&envelope.eip191_hash())
                .map_err(|e| e.into()),
        }
    }

    pub fn as_bytes(&self) -> [u8; 65] {
        match self {
            EnvelopeSignature::Secp256k1(sig) => sig.as_bytes(),
        }
    }
}

impl Packet {
    pub fn event_id(&self) -> EventId {
        self.envelope.eventId.into()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PacketRoute {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
}

impl PacketRoute {
    pub fn new_trigger_config(trigger_config: &TriggerConfig) -> Self {
        Self {
            service_id: trigger_config.service_id.clone(),
            workflow_id: trigger_config.workflow_id.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct EventId([u8; 20]);

impl From<FixedBytes<20>> for EventId {
    fn from(value: FixedBytes<20>) -> Self {
        Self(value.0)
    }
}

impl From<EventId> for FixedBytes<20> {
    fn from(value: EventId) -> Self {
        FixedBytes(value.0)
    }
}

impl TryFrom<&TriggerAction> for EventId {
    type Error = bincode::error::EncodeError;

    fn try_from(trigger_action: &TriggerAction) -> std::result::Result<EventId, Self::Error> {
        let bytes = bincode::encode_to_vec(trigger_action, bincode::config::standard())?;

        let mut hasher = Ripemd160::new();
        hasher.update(&bytes);
        let result = hasher.finalize();

        Ok(EventId(result.into()))
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", const_hex::encode(self.0))
    }
}

impl AsRef<[u8]> for EventId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct EventOrder([u8; 12]);

impl EventOrder {
    pub fn new_u64(value: u64) -> Self {
        let mut bytes = [0; 12];
        bytes[0..8].copy_from_slice(&value.to_be_bytes());
        Self(bytes)
    }
}

impl From<FixedBytes<12>> for EventOrder {
    fn from(value: FixedBytes<12>) -> Self {
        Self(value.0)
    }
}

impl From<EventOrder> for FixedBytes<12> {
    fn from(value: EventOrder) -> Self {
        FixedBytes(value.0)
    }
}

impl std::fmt::Display for EventOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", const_hex::encode(self.0))
    }
}

impl AsRef<[u8]> for EventOrder {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
