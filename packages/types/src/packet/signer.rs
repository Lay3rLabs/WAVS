pub use crate::solidity_types::Envelope;
use crate::{
    EnvelopeError, EnvelopeExt, EnvelopeSignature, SignatureAlgorithm, SignatureData,
    SignatureKind, SignaturePrefix,
};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait EnvelopeSigner: EnvelopeExt {
    async fn sign(
        &self,
        signer: &PrivateKeySigner,
        kind: SignatureKind,
    ) -> anyhow::Result<EnvelopeSignature> {
        let hash = match kind.algorithm {
            SignatureAlgorithm::Secp256k1 => match kind.prefix {
                Some(SignaturePrefix::Eip191) => self.prefix_eip191_hash(),
                None => self.unprefixed_hash(),
            },
        };

        Ok(signer
            .sign_hash(&hash)
            .await
            .map(|signature| EnvelopeSignature {
                data: signature.into(),
                kind,
            })
            .map_err(|e| anyhow::anyhow!("Failed to sign envelope: {e:?}"))?)
    }

    fn signature_data(
        &self,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, EnvelopeError> {
        let mut signers_and_signatures: Vec<(alloy_primitives::Address, alloy_primitives::Bytes)> =
            signatures
                .into_iter()
                .map(|sig| {
                    sig.evm_signer_address(self.borrow())
                        .map(|addr| (addr, sig.data.into()))
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

impl<T> EnvelopeSigner for T where T: EnvelopeExt {}

impl EnvelopeSignature {
    pub fn evm_signer_address(
        &self,
        envelope: &Envelope,
    ) -> std::result::Result<alloy_primitives::Address, EnvelopeError> {
        match self.kind.algorithm {
            SignatureAlgorithm::Secp256k1 => {
                let signature = alloy_primitives::Signature::from_raw(&self.data)
                    .map_err(EnvelopeError::RecoverSignerAddress)?;

                match self.kind.prefix {
                    Some(SignaturePrefix::Eip191) => signature
                        .recover_address_from_prehash(&envelope.prefix_eip191_hash())
                        .map_err(EnvelopeError::RecoverSignerAddress),
                    None => signature
                        .recover_address_from_prehash(&envelope.unprefixed_hash())
                        .map_err(EnvelopeError::RecoverSignerAddress),
                }
            }
        }
    }
}
