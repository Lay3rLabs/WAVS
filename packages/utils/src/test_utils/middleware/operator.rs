use alloy_primitives::Address;

#[derive(Clone, Debug)]
pub struct AvsOperator {
    pub operator: Address,
    pub signer: Address,
    pub weight: u64,
    pub operator_private_key: Option<String>,
    pub signer_private_key: Option<String>,
    /// 128-byte G1 public key (EIP-2537 uncompressed format) for BLS operators
    pub bls_pubkey: Option<Vec<u8>>,
    /// 256-byte G2 proof-of-possession for BLS operators
    pub bls_proof: Option<Vec<u8>>,
}

impl AvsOperator {
    pub const DEFAULT_WEIGHT: u64 = 10000;

    pub fn new(operator: Address, signer: Address) -> Self {
        Self {
            operator,
            signer,
            weight: Self::DEFAULT_WEIGHT,
            operator_private_key: None,
            signer_private_key: None,
            bls_pubkey: None,
            bls_proof: None,
        }
    }

    pub fn with_keys(
        operator: Address,
        signer: Address,
        operator_private_key: String,
        signer_private_key: String,
    ) -> Self {
        Self {
            operator,
            signer,
            weight: Self::DEFAULT_WEIGHT,
            operator_private_key: Some(operator_private_key),
            signer_private_key: Some(signer_private_key),
            bls_pubkey: None,
            bls_proof: None,
        }
    }

    /// Create an operator with BLS key material for BLS middleware registration.
    /// BLS operators use G1 pubkey + G2 proof instead of an EVM signer address.
    pub fn with_bls_keys(
        operator: Address,
        operator_private_key: String,
        bls_pubkey: Vec<u8>,
        bls_proof: Vec<u8>,
    ) -> Self {
        Self {
            operator,
            signer: Address::ZERO, // BLS doesn't use EVM signer address
            weight: Self::DEFAULT_WEIGHT,
            operator_private_key: Some(operator_private_key),
            signer_private_key: None,
            bls_pubkey: Some(bls_pubkey),
            bls_proof: Some(bls_proof),
        }
    }
}
