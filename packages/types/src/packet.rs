pub use crate::solidity_types::Envelope;
use crate::{
    ServiceID, ServiceManagerEnvelope, ServiceManagerSignatureData, SignatureData, TriggerAction,
    TriggerConfig, WorkflowID,
};
use alloy_primitives::{eip191_hash_message, keccak256, FixedBytes, SignatureError};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
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
    pub route: PacketRoute,
    #[schema(value_type  = Object)]
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

    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError>;
}

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
            operators: signature_data.operators,
            signatures: signature_data.signatures,
            referenceBlock: signature_data.referenceBlock,
        }
    }
}

impl EnvelopeExt for Envelope {
    fn eip191_hash(&self) -> FixedBytes<32> {
        let envelope_bytes = self.abi_encode();
        eip191_hash_message(keccak256(&envelope_bytes))
    }

    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError> {
        let mut operators_and_signatures: Vec<(
            alloy_primitives::Address,
            alloy_primitives::Bytes,
        )> = signatures
            .iter()
            .map(|sig| {
                sig.evm_signer_address(self)
                    .map(|addr| (addr, sig.as_bytes().into()))
            })
            .collect::<Result<_, _>>()?;

        // Solidity‑compatible ascending order (lexicographic / numeric)
        operators_and_signatures.sort_by_key(|(addr, _)| *addr);

        // unzip back into two parallel, sorted vectors
        let (operators, signatures): (
            Vec<alloy_primitives::Address>,
            Vec<alloy_primitives::Bytes>,
        ) = operators_and_signatures.into_iter().unzip();

        Ok(SignatureData {
            operators,
            signatures,
            referenceBlock: block_height as u32,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeSignature {
    #[schema(value_type = Object)]
    Secp256k1(alloy_primitives::Signature),
}

impl EnvelopeSignature {
    pub fn evm_signer_address(
        &self,
        envelope: &Envelope,
    ) -> std::result::Result<alloy_primitives::Address, EnvelopeError> {
        match self {
            EnvelopeSignature::Secp256k1(sig) => sig
                .recover_address_from_prehash(&envelope.eip191_hash())
                .map_err(EnvelopeError::RecoverSignerAddress),
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ToSchema)]
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

#[derive(
    Serialize, Deserialize, Clone, Eq, PartialEq, Debug, Hash, bincode::Decode, bincode::Encode,
)]
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

#[derive(Debug, Error)]
pub enum EnvelopeError {
    #[error("Unable to recover signer address: {0:?}")]
    RecoverSignerAddress(SignatureError),
}
