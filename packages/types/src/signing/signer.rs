use super::SignatureData;
use crate::{
    solidity_types::SignatureData as Secp256k1SignatureData, SignatureAlgorithm, SignatureKind,
    SignaturePrefix, SigningError, WavsSignable, WavsSignature,
};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;

#[cfg(feature = "bls")]
mod bls_helpers {
    //! BLS signing helpers for the WavsSigner::sign() BLS arm.
    //!
    //! These mirror `packages/utils/src/bls_signing.rs` (bls_sign_digest, bls_g1_pubkey_bytes).
    //! Duplication exists because wavs-types cannot depend on layer-utils (circular dep:
    //! layer-utils -> wavs-types). If refactoring the dep graph, consolidate these into a
    //! shared crate and remove this module.

    /// DST matching HashToCurve.sol line 20.
    /// MUST match packages/utils/src/bls_signing.rs::BLS_SIGNING_DST exactly.
    const BLS_SIGNING_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_";

    /// Sign a 32-byte digest, returning 256-byte EIP-2537 G2 signature.
    /// Mirrors utils::bls_signing::bls_sign_digest().
    pub(crate) fn bls_sign_digest_inner(
        private_key: &commonware_cryptography::bls12381::PrivateKey,
        digest: &[u8; 32],
    ) -> anyhow::Result<[u8; 256]> {
        use commonware_codec::Encode;
        let raw_bytes = private_key.encode();
        let sk = blst::min_pk::SecretKey::from_bytes(&raw_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to create blst SecretKey: {:?}", e))?;

        let signature = sk.sign(digest, BLS_SIGNING_DST, &[]);

        bls_g2_signature_bytes_inner(&signature)
    }

    /// Convert blst G2 signature to 256-byte EIP-2537 format.
    /// Mirrors utils::bls_signing::bls_g2_signature_bytes().
    fn bls_g2_signature_bytes_inner(
        signature: &blst::min_pk::Signature,
    ) -> anyhow::Result<[u8; 256]> {
        let uncompressed = signature.serialize(); // 192 bytes
        let mut eip2537 = [0u8; 256];
        for i in 0..4 {
            let src_offset = i * 48;
            let dst_offset = i * 64 + 16;
            eip2537[dst_offset..dst_offset + 48]
                .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
        }
        Ok(eip2537)
    }

    /// Get 128-byte EIP-2537 G1 public key from BLS private key.
    /// Mirrors utils::bls_signing::bls_g1_pubkey_bytes().
    pub(crate) fn bls_g1_pubkey_bytes_inner(
        private_key: &commonware_cryptography::bls12381::PrivateKey,
    ) -> anyhow::Result<[u8; 128]> {
        use commonware_cryptography::Signer as _;
        let pubkey = private_key.public_key();
        let compressed: &[u8] = &pubkey;

        let mut affine = blst::blst_p1_affine::default();
        // SAFETY: compressed is a valid 48-byte BLS public key from commonware
        let result = unsafe { blst::blst_p1_uncompress(&mut affine, compressed.as_ptr()) };
        if result != blst::BLST_ERROR::BLST_SUCCESS {
            anyhow::bail!("Failed to uncompress G1 point: {:?}", result);
        }
        let mut uncompressed_g1 = [0u8; 96];
        // SAFETY: affine is a valid P1 affine point, buffer is 96 bytes
        unsafe {
            blst::blst_p1_affine_serialize(uncompressed_g1.as_mut_ptr(), &affine);
        }

        let mut g1_eip2537 = [0u8; 128];
        g1_eip2537[16..64].copy_from_slice(&uncompressed_g1[0..48]);
        g1_eip2537[80..128].copy_from_slice(&uncompressed_g1[48..96]);

        Ok(g1_eip2537)
    }
}

/// Operator signing key supporting multiple signature algorithms.
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
            WavsCryptoSigner::Bls12381(ref bls_key) => {
                let hash = match kind.algorithm {
                    SignatureAlgorithm::Bls12381 => self.unprefixed_hash()?,
                    SignatureAlgorithm::Secp256k1 => {
                        anyhow::bail!("Cannot sign secp256k1 with a BLS key")
                    }
                };

                let digest: [u8; 32] = hash.into();
                let key = bls_key.clone();
                let kind = kind.clone();

                let (g2_sig, g1_pub) = tokio::task::spawn_blocking(move || {
                    let g2 = bls_helpers::bls_sign_digest_inner(&key, &digest)?;
                    let g1 = bls_helpers::bls_g1_pubkey_bytes_inner(&key)?;
                    Ok::<_, anyhow::Error>((g2, g1))
                })
                .await
                .map_err(|e| anyhow::anyhow!("BLS signing task failed: {e}"))??;

                Ok(WavsSignature::Bls12381 {
                    g2_signature: g2_sig.to_vec(),
                    g1_pubkey: g1_pub.to_vec(),
                    kind,
                })
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
