pub use crate::solidity_types::Envelope;
use crate::{ServiceID, TriggerAction, TriggerConfig, WorkflowID};
use alloy::primitives::FixedBytes;
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::Digest;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct Packet {
    pub route: PacketRoute,
    pub envelope: Envelope,
    // TODO - should this be pubkey or address?
    // it is used to check against operator set, so it's determined on the solidity side
    pub signer: SignerAddress,
    pub signature: Vec<u8>,
    pub block_height: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SignerAddress {
    Ethereum(alloy::primitives::Address),
    Cosmos(layer_climb_address::Address),
}

impl SignerAddress {
    pub fn eth_unchecked(&self) -> alloy::primitives::Address {
        match self {
            Self::Ethereum(addr) => *addr,
            _ => panic!("Expected signer address to be ethereum!"),
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
