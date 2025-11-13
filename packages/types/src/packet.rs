cfg_if::cfg_if! {
    if #[cfg(feature = "signer")] {
        pub mod signer;
        pub use signer::*;
    }
}

use std::borrow::Borrow;

pub use crate::solidity_types::Envelope;
use crate::{
    Service, ServiceId, ServiceManagerEnvelope, ServiceManagerSignatureData, SignatureData,
    SignatureKind, TriggerData, WorkflowId,
};
use alloy_primitives::{eip191_hash_message, keccak256, FixedBytes, SignatureError};
use alloy_sol_types::SolValue;
use async_trait::async_trait;
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct Packet {
    pub service: Service,
    pub workflow_id: WorkflowId,
    #[schema(value_type  = Object)]
    pub envelope: Envelope,
    pub signature: EnvelopeSignature,
    pub trigger_data: TriggerData,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait EnvelopeExt: Borrow<Envelope> {
    fn prefix_eip191_hash(&self) -> FixedBytes<32> {
        let envelope_bytes = self.borrow().abi_encode();
        eip191_hash_message(keccak256(&envelope_bytes))
    }

    fn unprefixed_hash(&self) -> FixedBytes<32> {
        let envelope_bytes = self.borrow().abi_encode();
        keccak256(&envelope_bytes)
    }
}

// Blanket impl for anything that borrows as Envelope
impl<T: Borrow<Envelope> + ?Sized> EnvelopeExt for T {}

impl From<Envelope> for ServiceManagerEnvelope {
    fn from(envelope: Envelope) -> Self {
        ServiceManagerEnvelope {
            eventId: envelope.eventId,
            ordering: envelope.ordering,
            payload: envelope.payload,
        }
    }
}

impl From<SignatureData> for ServiceManagerSignatureData {
    fn from(signature_data: SignatureData) -> Self {
        ServiceManagerSignatureData {
            signers: signature_data.signers,
            signatures: signature_data.signatures,
            referenceBlock: signature_data.referenceBlock,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct EnvelopeSignature {
    pub data: Vec<u8>,
    pub kind: SignatureKind,
}

impl Packet {
    pub fn event_id(&self) -> EventId {
        self.envelope.eventId.into()
    }
}

#[derive(
    Serialize,
    Deserialize,
    Clone,
    Eq,
    PartialEq,
    Debug,
    Hash,
    bincode::Decode,
    bincode::Encode,
    Ord,
    PartialOrd,
)]
#[serde(transparent)]
pub struct EventId(#[serde(with = "const_hex")] [u8; 20]);

impl EventId {
    pub fn new(
        service_id: &ServiceId,
        workflow_id: &WorkflowId,
        salt: EventIdSalt<'_>,
    ) -> Result<Self, bincode::error::EncodeError> {
        let mut hasher = Ripemd160::new();
        hasher.update(service_id.inner());
        hasher.update(workflow_id.as_bytes());
        match salt {
            EventIdSalt::WasmResponse(bytes) => hasher.update(bytes),
            EventIdSalt::Trigger(trigger_data) => hasher.update(bincode::serde::encode_to_vec(
                &trigger_data,
                bincode::config::standard(),
            )?),
        }
        let result = hasher.finalize();
        Ok(Self(result.into()))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

pub enum EventIdSalt<'a> {
    /// Raw data provided from the component
    WasmResponse(&'a [u8]),
    /// Data derived from the trigger
    Trigger(&'a TriggerData),
}

impl From<[u8; 20]> for EventId {
    fn from(value: [u8; 20]) -> Self {
        Self(value)
    }
}

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

#[derive(Debug, Error)]
pub enum EnvelopeError {
    #[error("Unable to recover signer address: {0:?}")]
    RecoverSignerAddress(SignatureError),
}
