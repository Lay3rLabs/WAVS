use alloy_primitives::Address;

#[derive(Clone, Debug)]
pub struct AvsOperator {
    pub operator: Address,
    pub signer: Address,
    pub weight: u64,
    pub operator_private_key: Option<String>,
    pub signer_private_key: Option<String>,
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
        }
    }
}
