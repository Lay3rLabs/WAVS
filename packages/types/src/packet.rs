use std::borrow::Borrow;

pub use crate::solidity_types::Envelope;
use crate::{
    Service, ServiceManagerEnvelope, ServiceManagerSignatureData, SignatureAlgorithm,
    SignatureData, SignatureKind, SignaturePrefix, TriggerAction, WorkflowId,
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
    pub service: Service,
    pub workflow_id: WorkflowId,
    #[schema(value_type  = Object)]
    pub envelope: Envelope,
    pub signature: EnvelopeSignature,
    // origin-* is used for reorg handling
    pub origin_tx_hash: Vec<u8>,
    pub origin_block: u64,
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

    async fn sign(
        &self,
        signer: &PrivateKeySigner,
        kind: SignatureKind,
    ) -> anyhow::Result<EnvelopeSignature> {
        let hash = match kind.algorithm {
            SignatureAlgorithm::Secp256k1 => match kind.prefix {
                Some(SignaturePrefix::Eip191) => self.prefix_eip191_hash(),
                None => self.unprefixed_hash(),
            },
        };

        Ok(signer
            .sign_hash(&hash)
            .await
            .map(|signature| EnvelopeSignature {
                data: signature.into(),
                kind,
            })
            .map_err(|e| anyhow::anyhow!("Failed to sign envelope: {e:?}"))?)
    }

    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError> {
        let mut signers_and_signatures: Vec<(alloy_primitives::Address, alloy_primitives::Bytes)> =
            signatures
                .into_iter()
                .map(|sig| {
                    sig.evm_signer_address(self.borrow())
                        .map(|addr| (addr, sig.data.into()))
                })
                .collect::<Result<_, _>>()?;

        // Solidity‑compatible ascending order (lexicographic / numeric)
        signers_and_signatures.sort_by_key(|(addr, _)| *addr);

        // unzip back into two parallel, sorted vectors
        let (signers, signatures): (Vec<alloy_primitives::Address>, Vec<alloy_primitives::Bytes>) =
            signers_and_signatures.into_iter().unzip();

        Ok(SignatureData {
            signers,
            signatures,
            referenceBlock: block_height as u32,
        })
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

impl EnvelopeSignature {
    pub fn evm_signer_address(
        &self,
        envelope: &Envelope,
    ) -> std::result::Result<alloy_primitives::Address, EnvelopeError> {
        match self.kind.algorithm {
            SignatureAlgorithm::Secp256k1 => {
                let signature = alloy_primitives::Signature::from_raw(&self.data)
                    .map_err(EnvelopeError::RecoverSignerAddress)?;

                match self.kind.prefix {
                    Some(SignaturePrefix::Eip191) => signature
                        .recover_address_from_prehash(&envelope.prefix_eip191_hash())
                        .map_err(EnvelopeError::RecoverSignerAddress),
                    None => signature
                        .recover_address_from_prehash(&envelope.unprefixed_hash())
                        .map_err(EnvelopeError::RecoverSignerAddress),
                }
            }
        }
    }
}

impl Packet {
    pub fn event_id(&self) -> EventId {
        self.envelope.eventId.into()
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

impl TryFrom<(&Service, &TriggerAction)> for EventId {
    type Error = anyhow::Error;

    fn try_from(
        (service, trigger_action): (&Service, &TriggerAction),
    ) -> std::result::Result<EventId, Self::Error> {
        let service_digest = service.hash()?;
        let action_bytes = bincode::encode_to_vec(trigger_action, bincode::config::standard())?;

        let mut hasher = Ripemd160::new();
        hasher.update(&service_digest);
        hasher.update(&action_bytes);
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
