use super::SignatureData;
use crate::{
    solidity_types::SignatureData as Secp256k1SignatureData, SignatureAlgorithm, SignatureKind,
    SignaturePrefix, SigningError, WavsSignable, WavsSignature,
};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;

/// Operator signing key supporting multiple signature algorithms.
/// BLS arm is defined in Phase 5 but signing logic is implemented in Phase 6.
#[derive(Clone)]
pub enum WavsCryptoSigner {
    Secp256k1(PrivateKeySigner),
    #[cfg(feature = "bls")]
    Bls12381(commonware_cryptography::bls12381::PrivateKey),
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait WavsSigner: WavsSignable {
    async fn sign(
        &self,
        signer: &WavsCryptoSigner,
        kind: SignatureKind,
    ) -> anyhow::Result<WavsSignature> {
        match signer {
            WavsCryptoSigner::Secp256k1(pks) => {
                let hash = match kind.algorithm {
                    SignatureAlgorithm::Secp256k1 => match kind.prefix {
                        Some(SignaturePrefix::Eip191) => self.prefix_eip191_hash()?,
                        None => self.unprefixed_hash()?,
                    },
                    SignatureAlgorithm::Bls12381 => {
                        anyhow::bail!("Cannot sign BLS with a secp256k1 key")
                    }
                };

                Ok(pks
                    .sign_hash(&hash)
                    .await
                    .map(|signature| WavsSignature::Secp256k1 {
                        data: signature.into(),
                        kind,
                    })
                    .map_err(|e| anyhow::anyhow!("Failed to sign data: {e:?}"))?)
            }
            #[cfg(feature = "bls")]
            WavsCryptoSigner::Bls12381(_) => {
                unimplemented!("BLS signing implemented in Phase 6")
            }
        }
    }

    fn signature_data(
        &self,
        signatures: Vec<WavsSignature>,
        block_height: u64,
    ) -> std::result::Result<SignatureData, SigningError> {
        // All signatures must be the same algorithm
        if signatures.is_empty() {
            return Err(SigningError::DataHash(anyhow::anyhow!(
                "No signatures provided"
            )));
        }

        // Check the first signature to determine algorithm
        match &signatures[0] {
            WavsSignature::Secp256k1 { .. } => {
                let mut signers_and_signatures: Vec<(
                    alloy_primitives::Address,
                    alloy_primitives::Bytes,
                )> = signatures
                    .into_iter()
                    .map(|sig| match sig {
                        WavsSignature::Secp256k1 { ref data, .. } => sig
                            .evm_signer_address(self)
                            .map(|addr| (addr, data.clone().into())),
                        WavsSignature::Bls12381 { .. } => Err(SigningError::DataHash(
                            anyhow::anyhow!("Mixed signature algorithms"),
                        )),
                    })
                    .collect::<Result<_, _>>()?;

                signers_and_signatures.sort_by_key(|(addr, _)| *addr);

                let (signers, signatures): (
                    Vec<alloy_primitives::Address>,
                    Vec<alloy_primitives::Bytes>,
                ) = signers_and_signatures.into_iter().unzip();

                Ok(SignatureData::Secp256k1(Secp256k1SignatureData {
                    signers,
                    signatures,
                    referenceBlock: block_height as u32,
                }))
            }
            WavsSignature::Bls12381 { .. } => {
                // BLS aggregation is implemented in Phase 7
                unimplemented!("BLS signature_data aggregation implemented in Phase 7")
            }
        }
    }
}

impl<T> WavsSigner for T where T: WavsSignable {}

impl WavsSignature {
    pub fn evm_signer_address<T: WavsSignable + ?Sized>(
        &self,
        signable: &T,
    ) -> std::result::Result<alloy_primitives::Address, SigningError> {
        match self {
            WavsSignature::Secp256k1 { data, kind } => {
                let signature = alloy_primitives::Signature::from_raw(data)
                    .map_err(SigningError::RecoverSignerAddress)?;

                match kind.prefix {
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
            WavsSignature::Bls12381 { .. } => Err(SigningError::DataHash(anyhow::anyhow!(
                "BLS signatures do not have EVM signer addresses"
            ))),
        }
    }
}
