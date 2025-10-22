use alloy_sol_types::SolValue;
use cosmwasm_schema::{cw_serde, QueryResponses};

/// To extend your contract so that it satisfies the `ServiceHandler` interface,  
/// include these messages in your contract's `QueryMsg` and `ExecuteMsg` enums
/// with the `#[serde(untagged)]` attribute
///
/// For example:
///
/// ```rust
/// use cosmwasm_schema::cw_serde;
/// use wavs_types::contracts::cosmwasm::service_handler::ServiceHandlerQueryMessages;
/// use wavs_types::contracts::cosmwasm::service_handler::ServiceHandlerExecuteMessages;
///
/// #[cw_serde]
/// #[schemaifier(mute_warnings)]
/// enum QueryMsg {
///     MyCustomMessage {
///         my_field: String,
///     },
///
///     #[serde(untagged)]
///     ServiceHandler(ServiceHandlerQueryMessages),
/// }
///
/// #[cw_serde]
/// #[schemaifier(mute_warnings)]
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
/// without needing to know your full `QueryMsg` or `ExecuteMsg` types
#[cw_serde]
pub enum ServiceHandlerExecuteMessages {
    WavsHandleSignedEnvelope {
        envelope: WavsEnvelope,
        signature_data: WavsSignatureData,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ServiceHandlerQueryMessages {
    /// Get the service manager address
    #[returns(cosmwasm_std::Addr)]
    WavsServiceManager {},
}

/// The `Envelope` from the Solidity interface, ABI-encoded into raw bytes
#[cw_serde]
pub struct WavsEnvelope(cosmwasm_std::Binary);

impl WavsEnvelope {
    pub fn new(envelope: crate::solidity_types::Envelope) -> Self {
        Self::new_raw(envelope.abi_encode())
    }

    pub fn new_raw(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn decode(&self) -> Result<crate::solidity_types::Envelope, alloy_sol_types::Error> {
        crate::solidity_types::Envelope::abi_decode(self.as_slice())
    }
}

impl From<crate::solidity_types::Envelope> for WavsEnvelope {
    fn from(envelope: crate::solidity_types::Envelope) -> Self {
        Self::new(envelope)
    }
}

/// A CosmWasm-friendly version of the `SignatureData` type from the Solidity interface
#[cw_serde]
pub struct WavsSignatureData {
    pub signers: Vec<layer_climb_address::AddrEvm>,
    pub signatures: Vec<cosmwasm_std::HexBinary>,
    pub reference_block: u32,
}

impl WavsSignatureData {
    pub fn new(signature_data: crate::solidity_types::SignatureData) -> Self {
        Self {
            signers: signature_data
                .signers
                .into_iter()
                .map(layer_climb_address::AddrEvm::from)
                .collect(),
            signatures: signature_data
                .signatures
                .into_iter()
                .map(|s| s.to_vec().into())
                .collect(),
            reference_block: signature_data.referenceBlock,
        }
    }
}

impl From<crate::solidity_types::SignatureData> for WavsSignatureData {
    fn from(signature_data: crate::solidity_types::SignatureData) -> Self {
        Self::new(signature_data)
    }
}

impl From<WavsSignatureData> for crate::solidity_types::SignatureData {
    fn from(signature_data: WavsSignatureData) -> Self {
        crate::solidity_types::SignatureData {
            signers: signature_data
                .signers
                .into_iter()
                .map(alloy_primitives::Address::from)
                .collect(),
            signatures: signature_data
                .signatures
                .into_iter()
                .map(|s| s.to_vec().into())
                .collect(),
            referenceBlock: signature_data.reference_block,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cw_serde]
    #[schemaifier(mute_warnings)]
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
            signers: vec![
                alloy_primitives::Address::new([42; 20]),
                alloy_primitives::Address::new([1; 20]),
            ],
            signatures: vec![
                alloy_primitives::Bytes::from(vec![1, 2, 3]),
                alloy_primitives::Bytes::from(vec![4, 5, 6]),
            ],
            referenceBlock: 12345,
        };

        // Create the messages for the service handler via .into()
        let msg_1 = ExampleServiceHandlerExecuteMsg::ServiceHandler(
            ServiceHandlerExecuteMessages::WavsHandleSignedEnvelope {
                envelope: envelope.into(),
                signature_data: signature_data.into(),
            },
        );
        const EXPECTED_MSG_1_STR:&str = "{\"wavs_handle_signed_envelope\":{\"envelope\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwECAwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",\"signature_data\":{\"signers\":[\"0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\",\"0x0101010101010101010101010101010101010101\"],\"signatures\":[\"010203\",\"040506\"],\"reference_block\":12345}}}";

        let msg_2 = ExampleServiceHandlerExecuteMsg::MyCustomMessage {
            my_field: "Hello".to_string(),
        };
        const EXPECTED_MSG_2_STR: &str = "{\"my_custom_message\":{\"my_field\":\"Hello\"}}";

        // The Wavs message gets a level removed, so we see the inner variant
        let serialized = serde_json::to_string(&msg_1).unwrap();
        assert_eq!(serialized, EXPECTED_MSG_1_STR);

        // The custom message is serialized with the outer level
        let serialized_2 = serde_json::to_string(&msg_2).unwrap();
        assert_eq!(serialized_2, EXPECTED_MSG_2_STR);

        // Can get back to the execution message from the serialized string in both cases

        let exec_msg: ExampleServiceHandlerExecuteMsg =
            serde_json::from_str(EXPECTED_MSG_1_STR).unwrap();

        assert!(matches!(
            exec_msg,
            ExampleServiceHandlerExecuteMsg::ServiceHandler(
                ServiceHandlerExecuteMessages::WavsHandleSignedEnvelope { .. }
            )
        ));

        let exec_msg: ExampleServiceHandlerExecuteMsg =
            serde_json::from_str(EXPECTED_MSG_2_STR).unwrap();

        assert!(matches!(
            exec_msg,
            ExampleServiceHandlerExecuteMsg::MyCustomMessage { .. }
        ));
    }
}
