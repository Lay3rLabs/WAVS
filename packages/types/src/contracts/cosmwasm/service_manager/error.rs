use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint256;

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
}
