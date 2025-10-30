use cosmwasm_schema::cw_serde;
use cosmwasm_std::{StdError, Uint256};

/// The possible errors that can occur during the validation of a signed envelope
#[cw_serde]
#[derive(thiserror::Error)]
pub enum WavsValidateError {
    #[error("Invalid signature length")]
    InvalidSignatureLength,
    #[error("Invalid signature block")]
    InvalidSignatureBlock,
    #[error("Invalid signature order")]
    InvalidSignatureOrder,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Insufficient quorum: zero signers")]
    InsufficientQuorumZero,
    #[error("Insufficient quorum: signer weight {signer_weight} is below threshold {threshold_weight} of total weight {total_weight}")]
    InsufficientQuorum {
        signer_weight: Uint256,
        threshold_weight: Uint256,
        total_weight: Uint256,
    },
    #[error("Invalid quorum parameters")]
    InvalidQuorumParameters,
    #[error("Registry not found")]
    MissingRegistry,
    #[error("Unable to decode envelope")]
    EnvelopeDecode,
    #[error("Could not parse signature")]
    SignatureParse,
}

impl TryFrom<StdError> for WavsValidateError {
    type Error = StdError;

    fn try_from(err: StdError) -> Result<Self, Self::Error> {
        match err.downcast_ref::<WavsValidateError>() {
            Some(e) => Ok(e.clone()),
            None => Err(StdError::msg("Error is not `WavsValidateError`")),
        }
    }
}

#[cw_serde]
#[derive(thiserror::Error)]
pub enum WavsEventError {
    #[error("Unexpected event type: expected [{expected}], found [{found}]")]
    EventType { expected: String, found: String },
    #[error("Missing attribute [{attr_key}] in event [{event_type}]")]
    MissingAttribute {
        event_type: String,
        attr_key: String,
    },
    #[error("Missing attributes [{attr_keys:?}] in event [{event_type}]")]
    MissingAttributes {
        event_type: String,
        attr_keys: Vec<String>,
    },
    #[error("Unable to parse attribute key: [{attr_key}] value: [{attr_value}] in event [{event_type}]: {err}")]
    ParseAttribute {
        event_type: String,
        attr_key: String,
        attr_value: String,
        err: String,
    },
}
