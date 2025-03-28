use alloy::primitives::Uint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use crate::solidity_types::Envelope;
use crate::{ServiceID, TriggerAction, WorkflowID};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Packet {
    pub route: PacketRoute,
    pub envelope: Envelope,
    pub signer: SignerAddress,
    pub signature: Vec<u8>,
    pub block_height: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SignerAddress {
    Ethereum(alloy::primitives::Address),
    Cosmos(layer_climb_address::Address)
}

impl SignerAddress {
    pub fn eth_unchecked(&self) -> alloy::primitives::Address {
        match self {
            Self::Ethereum(addr) => *addr,
            _ => panic!("Expected signer address to be ethereum!")
        }
    }
}

impl Packet {
    pub fn event_id(&self) -> EventId {
        self.envelope.eventId.into()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct PacketRoute {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct EventId([u8;32]);

impl From<Uint<256, 4>> for EventId {
    fn from(value: Uint<256, 4>) -> Self {
        let mut arr = [0;32];
        arr.copy_from_slice(&value.as_le_bytes());
        Self(arr)
    }
}

impl From<EventId> for Uint<256, 4> {
    fn from(value: EventId) -> Self {
        Uint::from_le_bytes(value.0)
    }
}

impl TriggerAction {
    // TODO - ordering? is this the right source?
    pub fn into_event_id(trigger_action: TriggerAction) -> EventId {
        // TODO - something more efficient
        let bytes = serde_json::to_vec(&trigger_action).unwrap();

        let digest = Sha256::digest(&bytes);

        let mut arr = [0;32];
        arr.copy_from_slice(digest.as_slice());

        EventId(arr)
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", const_hex::encode(&self.0))
    }
}

impl AsRef<[u8]> for EventId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}