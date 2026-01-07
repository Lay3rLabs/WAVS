pub use crate::solidity_types::Envelope;
use crate::{
    SignatureAlgorithm, SignatureData, SignatureKind, SignaturePrefix, SigningError, WavsSignable,
    WavsSignature,
};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait WavsSigner: WavsSignable {
    async fn sign(
        &self,
        signer: &PrivateKeySigner,
        kind: SignatureKind,
    ) -> anyhow::Result<WavsSignature> {
        let hash = match kind.algorithm {
            SignatureAlgorithm::Secp256k1 => match kind.prefix {
                Some(SignaturePrefix::Eip191) => self.prefix_eip191_hash()?,
                None => self.unprefixed_hash()?,
            },
        };

        Ok(signer
            .sign_hash(&hash)
            .await
            .map(|signature| WavsSignature {
                data: signature.into(),
                kind,
            })
            .map_err(|e| anyhow::anyhow!("Failed to sign data: {e:?}"))?)
    }

    fn signature_data(
        &self,
        signatures: Vec<WavsSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, SigningError> {
        let mut signers_and_signatures: Vec<(alloy_primitives::Address, alloy_primitives::Bytes)> =
            signatures
                .into_iter()
                .map(|sig| {
                    sig.evm_signer_address(self)
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

impl<T> WavsSigner for T where T: WavsSignable {}

impl WavsSignature {
    pub fn evm_signer_address<T: WavsSignable + ?Sized>(
        &self,
        signable: &T,
    ) -> std::result::Result<alloy_primitives::Address, SigningError> {
        match self.kind.algorithm {
            SignatureAlgorithm::Secp256k1 => {
                let signature = alloy_primitives::Signature::from_raw(&self.data)
                    .map_err(SigningError::RecoverSignerAddress)?;

                match self.kind.prefix {
                    Some(SignaturePrefix::Eip191) => signature
                        .recover_address_from_prehash(
                            &signable
                                .prefix_eip191_hash()
                                .map_err(SigningError::DataHash)?,
                        )
                        .map_err(SigningError::RecoverSignerAddress),
                    None => signature
                        .recover_address_from_prehash(
                            &signable.unprefixed_hash().map_err(SigningError::DataHash)?,
                        )
                        .map_err(SigningError::RecoverSignerAddress),
                }
            }
        }
    }
}
