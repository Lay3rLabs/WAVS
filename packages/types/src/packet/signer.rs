use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;

use crate::{Envelope, EnvelopeError, EnvelopeExt, EnvelopeSignature, SignatureData};

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait EnvelopeSigner: EnvelopeExt {
    async fn sign(&self, signer: &PrivateKeySigner) -> alloy_signer::Result<EnvelopeSignature> {
        signer
            .sign_hash(&self.eip191_hash())
            .await
            .map(EnvelopeSignature::Secp256k1)
    }

    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError>;
}

impl EnvelopeSigner for Envelope {
    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError> {
        let mut signers_and_signatures: Vec<(alloy_primitives::Address, alloy_primitives::Bytes)> =
            signatures
                .iter()
                .map(|sig| {
                    sig.evm_signer_address(self)
                        .map(|addr| (addr, sig.as_bytes().into()))
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

impl EnvelopeSignature {
    pub fn evm_signer_address(
        &self,
        envelope: &Envelope,
    ) -> std::result::Result<alloy_primitives::Address, EnvelopeError> {
        match self {
            EnvelopeSignature::Secp256k1(sig) => sig
                .recover_address_from_prehash(&envelope.eip191_hash())
                .map_err(EnvelopeError::RecoverSignerAddress),
        }
    }
}
