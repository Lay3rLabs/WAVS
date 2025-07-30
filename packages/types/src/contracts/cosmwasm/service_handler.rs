use cosmwasm_schema::cw_schema::{self, SchemaVisitor, Schemaifier};
use cosmwasm_schema::{cw_serde, schemars};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::{EventId, Ordering};

/// To extend your contract so that it satisfies the `ServiceHandler` interface,  
/// include these messages in your contract's `ExecuteMsg` enum
/// with the `#[serde(untagged)]` attribute
///
/// For example:
///
/// ```rust
/// use cosmwasm_schema::cw_serde;
/// use wavs_types::contracts::cosmwasm::service_handler::ServiceHandlerExecuteMessages;
///
/// #[cw_serde]
/// enum ExecuteMsg {
///     MyCustomMessage {
///         my_field: String,
///     },
///
///     #[serde(untagged)]
///     ServiceHandler(ServiceHandlerExecuteMessages),
/// }
/// ```
///
/// This allows WAVS to call your contract with the `ServiceHandler` messages,
/// without needing to know your full `ExecuteMsg` type
#[cw_serde]
pub enum ServiceHandlerExecuteMessages {
    WavsHandleSignedEnvelope {
        envelope: WavsEnvelope,
        signature_data: WavsSignatureData,
    },
}

/// A CosmWasm-friendly version of the `Envelope` type from the Solidity interface
#[cw_serde]
pub struct WavsEnvelope {
    pub event_id: EventId,
    pub ordering: Ordering,
    pub payload: Vec<u8>,
}

impl From<crate::solidity_types::Envelope> for WavsEnvelope {
    fn from(envelope: crate::solidity_types::Envelope) -> Self {
        WavsEnvelope {
            event_id: envelope.eventId.into(),
            ordering: envelope.ordering.into(),
            payload: envelope.payload.to_vec(),
        }
    }
}

impl From<WavsEnvelope> for crate::solidity_types::Envelope {
    fn from(envelope: WavsEnvelope) -> Self {
        crate::solidity_types::Envelope {
            eventId: envelope.event_id.into(),
            ordering: envelope.ordering.into(),
            payload: envelope.payload.into(),
        }
    }
}

/// A CosmWasm-friendly version of the `SignatureData` type from the Solidity interface
#[cw_serde]
pub struct WavsSignatureData {
    pub signers: Vec<EvmAddress>,
    pub signatures: Vec<Vec<u8>>,
    pub reference_block: u32,
}

/// A CosmWasm-friendly version of the `Address` type from the Solidity interface
#[derive(
    Serialize, Deserialize, Clone, Eq, PartialEq, Debug, Hash, bincode::Decode, bincode::Encode,
)]
#[cfg(feature = "cosmwasm")]
#[derive(schemars::JsonSchema)]
#[serde(transparent)]
pub struct EvmAddress([u8; 20]);

impl EvmAddress {
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

impl From<alloy_primitives::Address> for EvmAddress {
    fn from(value: alloy_primitives::Address) -> Self {
        Self(*value.0)
    }
}

impl From<EvmAddress> for alloy_primitives::Address {
    fn from(value: EvmAddress) -> Self {
        alloy_primitives::Address::new(value.0)
    }
}

impl From<crate::solidity_types::SignatureData> for WavsSignatureData {
    fn from(signature: crate::solidity_types::SignatureData) -> Self {
        WavsSignatureData {
            signers: signature
                .signers
                .into_iter()
                .map(EvmAddress::from)
                .collect(),
            signatures: signature
                .signatures
                .into_iter()
                .map(|s| s.to_vec())
                .collect(),
            reference_block: signature.referenceBlock,
        }
    }
}

impl From<WavsSignatureData> for crate::solidity_types::SignatureData {
    fn from(signature: WavsSignatureData) -> Self {
        crate::solidity_types::SignatureData {
            signers: signature
                .signers
                .into_iter()
                .map(alloy_primitives::Address::from)
                .collect(),
            signatures: signature
                .signatures
                .into_iter()
                .map(alloy_primitives::Bytes::from)
                .collect(),
            referenceBlock: signature.reference_block,
        }
    }
}

impl Schemaifier for EvmAddress {
    #[inline]
    fn visit_schema(visitor: &mut SchemaVisitor) -> cw_schema::DefinitionReference {
        let node = cw_schema::Node {
            name: Cow::Borrowed(std::any::type_name::<Self>()),
            description: None,
            value: cw_schema::NodeType::Array {
                items: u8::visit_schema(visitor),
            },
        };

        visitor.insert(Self::id(), node)
    }
}

impl Schemaifier for EventId {
    #[inline]
    fn visit_schema(visitor: &mut SchemaVisitor) -> cw_schema::DefinitionReference {
        let node = cw_schema::Node {
            name: Cow::Borrowed(std::any::type_name::<Self>()),
            description: None,
            value: cw_schema::NodeType::Array {
                items: u8::visit_schema(visitor),
            },
        };

        visitor.insert(Self::id(), node)
    }
}

impl Schemaifier for Ordering {
    #[inline]
    fn visit_schema(visitor: &mut SchemaVisitor) -> cw_schema::DefinitionReference {
        let node = cw_schema::Node {
            name: Cow::Borrowed(std::any::type_name::<Self>()),
            description: None,
            value: cw_schema::NodeType::Array {
                items: u8::visit_schema(visitor),
            },
        };

        visitor.insert(Self::id(), node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cw_serde]
    enum ExampleServiceHandlerExecuteMsg {
        MyCustomMessage {
            my_field: String,
        },
        #[serde(untagged)]
        ServiceHandler(ServiceHandlerExecuteMessages),
    }

    #[test]
    fn service_handler_execute_msg_flatten() {
        // Assume our source of data comes from a Solidity contract
        let envelope = crate::solidity_types::Envelope {
            eventId: [0; 20].into(),
            ordering: [0; 12].into(),
            payload: vec![1, 2, 3].into(),
        };

        let signature_data = crate::solidity_types::SignatureData {
            signers: vec![alloy_primitives::Address::new([0; 20])],
            signatures: vec![alloy_primitives::Bytes::from(vec![4, 5, 6])],
            referenceBlock: 12345,
        };

        // Create the messages for the service handler via .into()
        let msg_1 = ExampleServiceHandlerExecuteMsg::ServiceHandler(
            ServiceHandlerExecuteMessages::WavsHandleSignedEnvelope {
                envelope: envelope.into(),
                signature_data: signature_data.into(),
            },
        );

        let msg_2 = ExampleServiceHandlerExecuteMsg::MyCustomMessage {
            my_field: "Hello".to_string(),
        };

        // The Wavs message gets a level removed, so we see the inner variant
        let serialized = serde_json::to_string(&msg_1).unwrap();
        assert_eq!(serialized, "{\"wavs_handle_signed_envelope\":{\"envelope\":{\"event_id\":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],\"ordering\":[0,0,0,0,0,0,0,0,0,0,0,0],\"payload\":[1,2,3]},\"signature_data\":{\"signers\":[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]],\"signatures\":[[4,5,6]],\"reference_block\":12345}}}");

        // The custom message is serialized with the outer level
        let serialized_2 = serde_json::to_string(&msg_2).unwrap();
        assert_eq!(
            serialized_2,
            "{\"my_custom_message\":{\"my_field\":\"Hello\"}}"
        );
    }
}
