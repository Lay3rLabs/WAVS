pub mod error;
pub mod event;

use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::StdResult;
use layer_climb_address::EvmAddr;

use crate::contracts::cosmwasm::{
    service_handler::{WavsEnvelope, WavsSignatureData},
    service_manager::error::WavsValidateError,
};

/// To extend your contract so that it satisfies the `ServiceManager` interface,
/// include these messages in your contract's `QueryMsg` and `ExecuteMsg` enums
/// with the `#[serde(untagged)]` attribute
///
/// For example:
///
/// ```rust
/// use cosmwasm_schema::cw_serde;
/// use wavs_types::contracts::cosmwasm::service_manager::ServiceManagerQueryMessages;
/// use wavs_types::contracts::cosmwasm::service_manager::ServiceManagerExecuteMessages;
///
/// #[cw_serde]
/// #[schemaifier(mute_warnings)]
/// enum QueryMsg {
///     MyCustomMessage {
///         my_field: String,
///     },
///
///     #[serde(untagged)]
///     ServiceManager(ServiceManagerQueryMessages),
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
///     ServiceManager(ServiceManagerExecuteMessages),
/// }
/// ```
///
/// This allows WAVS to call your contract with the `ServiceManager` messages,
/// without needing to know your full `QueryMsg` or `ExecuteMsg` types
#[cw_serde]
pub enum ServiceManagerExecuteMessages {
    /// Set the service URI for the WAVS service manager
    WavsSetServiceUri { service_uri: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum ServiceManagerQueryMessages {
    /// Get the given operator's current weight
    #[returns(cosmwasm_std::Uint256)]
    WavsOperatorWeight { operator_address: EvmAddr },

    /// Validate a signed envelope
    #[returns(WavsValidateResult)]
    WavsValidate {
        envelope: WavsEnvelope,
        signature_data: WavsSignatureData,
    },

    /// Get the service URI
    #[returns(String)]
    WavsServiceUri {},

    /// Get the latest operator address for a given signing key address
    #[returns(Option<EvmAddr>)]
    WavsLatestOperatorForSigningKey { signing_key_addr: EvmAddr },
}

/// The result of validating a signed envelope
// TODO: make `Try` once it's stable: https://doc.rust-lang.org/std/ops/trait.Try.html
#[cw_serde]
pub enum WavsValidateResult {
    Ok,
    Err(WavsValidateError),
}

impl WavsValidateResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, WavsValidateResult::Ok)
    }

    pub fn is_err(&self) -> bool {
        matches!(self, WavsValidateResult::Err(_))
    }

    pub fn into_std(self) -> StdResult<()> {
        self.into()
    }
}

impl From<WavsValidateResult> for StdResult<()> {
    fn from(result: WavsValidateResult) -> Self {
        match result {
            WavsValidateResult::Ok => Ok(()),
            WavsValidateResult::Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cw_serde]
    #[schemaifier(mute_warnings)]
    enum ExampleServiceManagerExecuteMsg {
        MyCustomMessage {
            my_field: String,
        },
        #[serde(untagged)]
        ServiceManager(ServiceManagerExecuteMessages),
    }

    #[cw_serde]
    #[schemaifier(mute_warnings)]
    enum ExampleServiceManagerQueryMsg {
        MyCustomMessage {
            my_field: String,
        },
        #[serde(untagged)]
        ServiceManager(ServiceManagerQueryMessages),
    }

    #[test]
    fn service_manager_execute_msg_flatten() {
        let service_uri = "https://example.com/service".to_string();

        // Create the messages for the service handler via .into()
        let msg_1 = ExampleServiceManagerExecuteMsg::ServiceManager(
            ServiceManagerExecuteMessages::WavsSetServiceUri {
                service_uri: service_uri.to_string(),
            },
        );
        let expected_msg_1 =
            format!(r#"{{"wavs_set_service_uri":{{"service_uri":"{service_uri}"}}}}"#);

        let msg_2 = ExampleServiceManagerExecuteMsg::MyCustomMessage {
            my_field: "Hello".to_string(),
        };
        const EXPECTED_MSG_2_STR: &str = "{\"my_custom_message\":{\"my_field\":\"Hello\"}}";

        // The Wavs message gets a level removed, so we see the inner variant
        let serialized = serde_json::to_string(&msg_1).unwrap();
        assert_eq!(serialized, expected_msg_1);

        // The custom message is serialized with the outer level
        let serialized_2 = serde_json::to_string(&msg_2).unwrap();
        assert_eq!(serialized_2, EXPECTED_MSG_2_STR);

        // Can get back to the execution message from the serialized string in both cases

        let exec_msg: ExampleServiceManagerExecuteMsg =
            serde_json::from_str(&expected_msg_1).unwrap();

        assert!(matches!(
            exec_msg,
            ExampleServiceManagerExecuteMsg::ServiceManager(
                ServiceManagerExecuteMessages::WavsSetServiceUri { .. }
            )
        ));

        let exec_msg: ExampleServiceManagerExecuteMsg =
            serde_json::from_str(EXPECTED_MSG_2_STR).unwrap();

        assert!(matches!(
            exec_msg,
            ExampleServiceManagerExecuteMsg::MyCustomMessage { .. }
        ));
    }

    #[test]
    fn service_manager_query_msg_flatten() {
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

        // Create the messages for the service manger via .into()
        let msg_1 = ExampleServiceManagerQueryMsg::ServiceManager(
            ServiceManagerQueryMessages::WavsValidate {
                envelope: envelope.into(),
                signature_data: signature_data.into(),
            },
        );
        const EXPECTED_MSG_1_STR:&str = "{\"wavs_validate\":{\"envelope\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwECAwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",\"signature_data\":{\"signers\":[\"0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\",\"0x0101010101010101010101010101010101010101\"],\"signatures\":[\"010203\",\"040506\"],\"reference_block\":12345}}}";

        let msg_2 = ExampleServiceManagerQueryMsg::MyCustomMessage {
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

        let query_msg: ExampleServiceManagerQueryMsg =
            serde_json::from_str(EXPECTED_MSG_1_STR).unwrap();

        assert!(matches!(
            query_msg,
            ExampleServiceManagerQueryMsg::ServiceManager(
                ServiceManagerQueryMessages::WavsValidate { .. }
            )
        ));

        let query_msg: ExampleServiceManagerQueryMsg =
            serde_json::from_str(EXPECTED_MSG_2_STR).unwrap();

        assert!(matches!(
            query_msg,
            ExampleServiceManagerQueryMsg::MyCustomMessage { .. }
        ));
    }
}
