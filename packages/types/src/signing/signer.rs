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
    ///
    /// blst (ZCash) serializes G2 as 192 bytes with the imaginary part first:
    ///   [x_c1(48)] [x_c0(48)] [y_c1(48)] [y_c0(48)]
    /// EIP-2537 expects the real part first, each Fp padded to 64 bytes:
    ///   [x_c0(64)] [x_c1(64)] [y_c0(64)] [y_c1(64)]
    fn bls_g2_signature_bytes_inner(
        signature: &blst::min_pk::Signature,
    ) -> anyhow::Result<[u8; 256]> {
        let uncompressed = signature.serialize(); // 192 bytes: x_c1|x_c0|y_c1|y_c0 (ZCash)
        let mut eip2537 = [0u8; 256];
        // blst source offsets for [x_c0, x_c1, y_c0, y_c1] (EIP-2537 order):
        //   x_c0 is at blst[48..96], x_c1 is at blst[0..48]
        //   y_c0 is at blst[144..192], y_c1 is at blst[96..144]
        let blst_src_offsets = [48usize, 0, 144, 96];
        for (i, &src_offset) in blst_src_offsets.iter().enumerate() {
            let dst_offset = i * 64 + 16;
            eip2537[dst_offset..dst_offset + 48]
                .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
        }
        Ok(eip2537)
    }

    /// Deserialize a 256-byte EIP-2537 G2 signature back to a blst Signature.
    /// Strips the 16-byte zero padding and reverses the coordinate swap to get
    /// 192-byte ZCash/blst uncompressed form, then calls Signature::deserialize().
    pub(crate) fn deserialize_g2_from_eip2537(
        eip2537_bytes: &[u8],
    ) -> anyhow::Result<blst::min_pk::Signature> {
        if eip2537_bytes.len() != 256 {
            anyhow::bail!("Expected 256-byte EIP-2537 G2, got {}", eip2537_bytes.len());
        }
        let mut uncompressed = [0u8; 192];
        // EIP-2537 order: [x_c0, x_c1, y_c0, y_c1]; blst ZCash order: [x_c1, x_c0, y_c1, y_c0]
        // Map EIP-2537 position i to blst destination offset:
        //   EIP-2537[0]=x_c0 → blst[48..96], EIP-2537[1]=x_c1 → blst[0..48]
        //   EIP-2537[2]=y_c0 → blst[144..192], EIP-2537[3]=y_c1 → blst[96..144]
        let blst_dst_offsets = [48usize, 0, 144, 96];
        for (i, &dst_offset) in blst_dst_offsets.iter().enumerate() {
            let src_offset = i * 64 + 16;
            uncompressed[dst_offset..dst_offset + 48]
                .copy_from_slice(&eip2537_bytes[src_offset..src_offset + 48]);
        }
        blst::min_pk::Signature::deserialize(&uncompressed)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize G2 signature: {:?}", e))
    }

    /// Serialize a blst AggregateSignature to 256-byte EIP-2537 format.
    /// Converts 192-byte ZCash/blst uncompressed to 256-byte padded EIP-2537.
    pub(crate) fn serialize_aggregate_to_eip2537(
        aggregate: &blst::min_pk::AggregateSignature,
    ) -> [u8; 256] {
        let sig = aggregate.to_signature();
        let uncompressed = sig.serialize(); // 192 bytes: x_c1|x_c0|y_c1|y_c0 (ZCash)
        let mut eip2537 = [0u8; 256];
        // Same coordinate swap as bls_g2_signature_bytes_inner
        let blst_src_offsets = [48usize, 0, 144, 96];
        for (i, &src_offset) in blst_src_offsets.iter().enumerate() {
            let dst_offset = i * 64 + 16;
            eip2537[dst_offset..dst_offset + 48]
                .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
        }
        eip2537
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
            #[cfg(feature = "bls")]
            WavsSignature::Bls12381 { .. } => {
                use crate::solidity_types::BlsServiceHandler;
                use alloy_primitives::{keccak256, Bytes, FixedBytes};

                // Collect (keccak256_hash, g1_pubkey_bytes, deserialized_g2_sig) per operator
                let mut entries: Vec<(FixedBytes<32>, Bytes, blst::min_pk::Signature)> = signatures
                    .into_iter()
                    .map(|sig| match sig {
                        WavsSignature::Bls12381 {
                            g2_signature,
                            g1_pubkey,
                            ..
                        } => {
                            let key_hash = keccak256(&g1_pubkey);
                            let g2_sig = bls_helpers::deserialize_g2_from_eip2537(&g2_signature)
                                .map_err(SigningError::DataHash)?;
                            Ok((key_hash, Bytes::from(g1_pubkey), g2_sig))
                        }
                        WavsSignature::Secp256k1 { .. } => {
                            Err(SigningError::DataHash(anyhow::anyhow!(
                                "Mixed signature algorithms: expected BLS, got secp256k1"
                            )))
                        }
                    })
                    .collect::<Result<_, _>>()?;

                // Sort by keccak256(pubkey) ascending -- contract enforces lastKeyHash < keyHash
                entries.sort_by_key(|(hash, _, _)| *hash);

                // Aggregate G2 signatures via blst point addition
                let sig_refs: Vec<&blst::min_pk::Signature> =
                    entries.iter().map(|(_, _, s)| s).collect();
                let aggregate = blst::min_pk::AggregateSignature::aggregate(&sig_refs, true)
                    .map_err(|e| {
                        SigningError::DataHash(anyhow::anyhow!("BLS aggregate failed: {:?}", e))
                    })?;
                let agg_sig_bytes = bls_helpers::serialize_aggregate_to_eip2537(&aggregate);

                let signer_pubkeys: Vec<Bytes> = entries.into_iter().map(|(_, pk, _)| pk).collect();

                Ok(SignatureData::Bls12381(BlsServiceHandler::SignatureData {
                    signerPubkeys: signer_pubkeys,
                    aggregateSignature: Bytes::from(agg_sig_bytes.to_vec()),
                    referenceBlock: block_height as u32,
                }))
            }
            #[cfg(not(feature = "bls"))]
            WavsSignature::Bls12381 { .. } => Err(SigningError::DataHash(anyhow::anyhow!(
                "BLS aggregation requires the 'bls' feature"
            ))),
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

#[cfg(all(test, feature = "bls"))]
mod tests {
    use super::*;
    use crate::{SignatureAlgorithm, WavsSignable};
    use alloy_primitives::keccak256;

    /// Minimal signable for tests -- just wraps raw bytes.
    struct TestSignable(Vec<u8>);
    impl WavsSignable for TestSignable {
        fn encode_data(&self) -> anyhow::Result<Vec<u8>> {
            Ok(self.0.clone())
        }
    }

    /// Helper: generate a BLS key pair, sign a digest, return (g2_sig_bytes, g1_pubkey_bytes).
    fn make_bls_signature(seed: u64) -> (Vec<u8>, Vec<u8>) {
        use commonware_math::algebra::Random;
        use rand_chacha::rand_core::SeedableRng;
        // Create deterministic key from seed -- need rand_core 0.6 RNG for commonware
        let mut seed_bytes = [0u8; 32];
        seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(seed_bytes);
        let key = commonware_cryptography::bls12381::PrivateKey::random(&mut rng);

        let digest = [0xabu8; 32]; // fixed test digest
        let g2_sig = bls_helpers::bls_sign_digest_inner(&key, &digest).unwrap();
        let g1_pub = bls_helpers::bls_g1_pubkey_bytes_inner(&key).unwrap();
        (g2_sig.to_vec(), g1_pub.to_vec())
    }

    fn bls_kind() -> SignatureKind {
        SignatureKind {
            algorithm: SignatureAlgorithm::Bls12381,
            prefix: None,
        }
    }

    #[test]
    fn bls_signature_data_aggregates_g2() {
        let signable = TestSignable(vec![1, 2, 3]);
        let sigs: Vec<WavsSignature> = (1..=3u64)
            .map(|seed| {
                let (g2, g1) = make_bls_signature(seed);
                WavsSignature::Bls12381 {
                    g2_signature: g2,
                    g1_pubkey: g1,
                    kind: bls_kind(),
                }
            })
            .collect();

        let result = signable.signature_data(sigs, 42);
        assert!(
            result.is_ok(),
            "signature_data must succeed: {:?}",
            result.err()
        );
        match result.unwrap() {
            SignatureData::Bls12381(data) => {
                assert_eq!(
                    data.aggregateSignature.len(),
                    256,
                    "Aggregate sig must be 256 bytes (EIP-2537)"
                );
                assert_eq!(data.signerPubkeys.len(), 3, "Must have 3 signer pubkeys");
                for pk in &data.signerPubkeys {
                    assert_eq!(pk.len(), 128, "Each G1 pubkey must be 128 bytes (EIP-2537)");
                }
                assert_eq!(data.referenceBlock, 42);
            }
            _ => panic!("Expected Bls12381 variant"),
        }
    }

    #[test]
    fn bls_signature_data_sorts_by_keccak() {
        let signable = TestSignable(vec![1, 2, 3]);
        let sigs: Vec<WavsSignature> = (1..=3u64)
            .map(|seed| {
                let (g2, g1) = make_bls_signature(seed);
                WavsSignature::Bls12381 {
                    g2_signature: g2,
                    g1_pubkey: g1,
                    kind: bls_kind(),
                }
            })
            .collect();

        let result = signable.signature_data(sigs, 100).unwrap();
        match result {
            SignatureData::Bls12381(data) => {
                // Verify pubkeys are sorted by keccak256(pubkey) ascending
                let hashes: Vec<_> = data
                    .signerPubkeys
                    .iter()
                    .map(|pk| keccak256(pk.as_ref()))
                    .collect();
                for i in 1..hashes.len() {
                    assert!(
                        hashes[i - 1] < hashes[i],
                        "Pubkeys must be sorted by keccak256 ascending: {:?} >= {:?}",
                        hashes[i - 1],
                        hashes[i]
                    );
                }
            }
            _ => panic!("Expected Bls12381 variant"),
        }
    }

    #[test]
    fn bls_signature_data_rejects_mixed() {
        let signable = TestSignable(vec![1, 2, 3]);
        let (g2, g1) = make_bls_signature(1);
        let sigs = vec![
            WavsSignature::Bls12381 {
                g2_signature: g2,
                g1_pubkey: g1,
                kind: bls_kind(),
            },
            WavsSignature::Secp256k1 {
                data: vec![0u8; 65],
                kind: SignatureKind::evm_default(),
            },
        ];

        let result = signable.signature_data(sigs, 50);
        assert!(result.is_err(), "Mixed algorithms must fail");
    }

    #[test]
    fn bls_signature_data_empty_rejects() {
        let signable = TestSignable(vec![1, 2, 3]);
        let result = signable.signature_data(vec![], 50);
        assert!(result.is_err(), "Empty signatures must fail");
    }

    // bls_rpc_bindings_compile test is in bls.rs tests module (requires solidity-rpc feature)
}
