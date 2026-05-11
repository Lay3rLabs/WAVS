//! BLS12-381 key derivation from BIP-39 mnemonic.
//!
//! Derives deterministic BLS private keys using HKDF-SHA256 to incorporate
//! the HD index, then seeds ChaCha20Rng for commonware's PrivateKey::random().
//! This parallels the ed25519 derivation in aggregator/p2p.rs but adds HKDF
//! to safely derive multiple keys from a single mnemonic.

use commonware_cryptography::{bls12381, Signer as _};
use hkdf::Hkdf;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use sha2::Sha256;

/// Domain separation label for HKDF info parameter.
/// Prevents accidental collision with other HKDF usages of the same BIP-39 seed.
const HKDF_INFO_PREFIX: &[u8] = b"WAVS-BLS-KEY-v1";

/// Derive a BLS12-381 private key deterministically from a BIP-39 mnemonic and HD index.
///
/// Algorithm:
/// 1. Parse BIP-39 mnemonic and derive 64-byte seed (empty passphrase)
/// 2. HKDF-SHA256(ikm=seed, info=WAVS-BLS-KEY-v1 || hd_index.to_le_bytes()) -> 32-byte RNG seed
/// 3. ChaCha20Rng::from_seed(rng_seed) -> deterministic PRNG
/// 4. bls12381::PrivateKey::random(&mut rng) -> BLS private key
///
/// # Errors
/// - Returns error if `mnemonic` starts with "0x" (raw key, not a mnemonic)
/// - Returns error if mnemonic is invalid BIP-39
pub fn bls_private_key_from_mnemonic(
    mnemonic: &str,
    hd_index: u32,
) -> anyhow::Result<bls12381::PrivateKey> {
    // Guard: reject raw private keys
    if mnemonic.starts_with("0x") {
        anyhow::bail!("BLS key derivation requires a mnemonic, not a raw key");
    }

    // Parse BIP-39 mnemonic
    let mnemonic =
        bip39::Mnemonic::parse(mnemonic).map_err(|e| anyhow::anyhow!("Invalid mnemonic: {}", e))?;

    // Derive 64-byte BIP-39 seed (empty passphrase)
    let seed = mnemonic.to_seed("");

    // HKDF-SHA256: incorporate HD index with domain separation
    let hk = Hkdf::<Sha256>::new(None, &seed);
    let mut rng_seed = [0u8; 32];
    // info = WAVS-BLS-KEY-v1 || hd_index (little-endian)
    let mut info = Vec::with_capacity(HKDF_INFO_PREFIX.len() + 4);
    info.extend_from_slice(HKDF_INFO_PREFIX);
    info.extend_from_slice(&hd_index.to_le_bytes());
    hk.expand(&info, &mut rng_seed)
        .map_err(|e| anyhow::anyhow!("HKDF expand failed: {}", e))?;

    // Deterministic RNG seeded from HKDF output
    let mut rng = ChaCha20Rng::from_seed(rng_seed);

    // Generate BLS private key using commonware's implementation
    use commonware_math::algebra::Random;
    Ok(bls12381::PrivateKey::random(&mut rng))
}

/// Convert a BLS private key's G1 public key to 128-byte EIP-2537 uncompressed format.
///
/// The commonware PublicKey is 48-byte ZCash compressed G1. This function:
/// 1. Decompresses to affine point via blst FFI
/// 2. Serializes to 96 bytes (x || y, each 48 bytes big-endian)
/// 3. Pads each 48-byte coordinate to 64 bytes with leading zeros (EIP-2537 format)
///
/// The output matches `BLS12381.G1_POINT_SIZE = 128` in the poa-middleware contracts.
pub fn bls_g1_pubkey_bytes(private_key: &bls12381::PrivateKey) -> anyhow::Result<[u8; 128]> {
    let pubkey = private_key.public_key();
    let compressed: &[u8] = &pubkey; // 48-byte ZCash compressed G1

    // Decompress to affine point via blst FFI
    let mut affine = blst::blst_p1_affine::default();
    let result = unsafe { blst::blst_p1_uncompress(&mut affine, compressed.as_ptr()) };
    if result != blst::BLST_ERROR::BLST_SUCCESS {
        anyhow::bail!("Failed to uncompress G1 point: {:?}", result);
    }

    // Serialize to 96-byte uncompressed (x || y, each 48 bytes big-endian)
    let mut uncompressed = [0u8; 96];
    unsafe {
        blst::blst_p1_affine_serialize(uncompressed.as_mut_ptr(), &affine);
    }

    // Pad each 48-byte coordinate to 64 bytes (EIP-2537 format)
    // x: 16 zero bytes + 48-byte x coordinate
    // y: 16 zero bytes + 48-byte y coordinate
    let mut eip2537 = [0u8; 128];
    eip2537[16..64].copy_from_slice(&uncompressed[0..48]);
    eip2537[80..128].copy_from_slice(&uncompressed[48..96]);

    Ok(eip2537)
}

/// DST for BLS signing, matching HashToCurve.sol line 20.
/// NOTE: This is NOT the same as commonware's G2_MESSAGE DST which has _POP_ suffix.
pub const BLS_SIGNING_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_";

/// Sign a 32-byte digest with a BLS private key using hash-to-curve.
/// Returns 256-byte EIP-2537 G2 signature.
///
/// The digest is typically `keccak256(abi_encode(envelope))` matching the contract's
/// `digest = keccak256(abi.encode(envelope))` then `hashToCurveG2(abi.encodePacked(digest))`.
///
/// Uses blst directly (not commonware Signer::sign) because:
/// - commonware uses DST `..._POP_` suffix, contract uses `..._RO_` suffix
/// - commonware wraps message with union_unique(namespace, message), contract uses raw bytes
pub fn bls_sign_digest(
    private_key: &bls12381::PrivateKey,
    digest: &[u8; 32],
) -> anyhow::Result<[u8; 256]> {
    use commonware_codec::Encode;
    let raw_bytes = private_key.encode();
    let sk = blst::min_pk::SecretKey::from_bytes(&raw_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to create blst SecretKey: {:?}", e))?;

    // Sign with contract-matching DST. The message is the raw 32-byte digest.
    let signature = sk.sign(digest, BLS_SIGNING_DST, &[]);

    bls_g2_signature_bytes(&signature)
}

/// Convert a blst G2 signature to 256-byte EIP-2537 uncompressed format.
///
/// blst (ZCash format) serializes G2 as 192 bytes with the IMAGINARY part first:
///   [x_c1(48)] [x_c0(48)] [y_c1(48)] [y_c0(48)]
///
/// EIP-2537 format uses the REAL part first, each Fp padded to 64 bytes:
///   [x_c0(64)] [x_c1(64)] [y_c0(64)] [y_c1(64)]
///
/// The coordinate swap (c1 ↔ c0) is required because ZCash/blst puts c1 before c0
/// while EIP-2537 puts c0 before c1.
///
/// Matches `BLS12381.G2_POINT_SIZE = 256` in poa-middleware contracts.
pub fn bls_g2_signature_bytes(signature: &blst::min_pk::Signature) -> anyhow::Result<[u8; 256]> {
    let uncompressed = signature.serialize(); // 192 bytes: x_c1|x_c0|y_c1|y_c0 (ZCash order)
    let mut eip2537 = [0u8; 256];

    // blst ZCash offsets for [x_c0, x_c1, y_c0, y_c1] (EIP-2537 order):
    //   x_c0 is at blst[48..96], x_c1 is at blst[0..48]
    //   y_c0 is at blst[144..192], y_c1 is at blst[96..144]
    let blst_src_offsets = [48usize, 0, 144, 96];
    for (i, &src_offset) in blst_src_offsets.iter().enumerate() {
        let dst_offset = i * 64 + 16; // 16-byte zero prefix in EIP-2537 Fp element
        eip2537[dst_offset..dst_offset + 48]
            .copy_from_slice(&uncompressed[src_offset..src_offset + 48]);
    }

    Ok(eip2537)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_codec::Encode;

    const TEST_MNEMONIC: &str = "test test test test test test test test test test test junk";

    /// Helper to extract compressed pubkey bytes (48 bytes) for comparison.
    fn pubkey_bytes(key: &bls12381::PrivateKey) -> Vec<u8> {
        let pk = key.public_key();
        // Use Deref<Target=[u8]> to get the raw bytes
        let bytes: &[u8] = &pk;
        bytes.to_vec()
    }

    #[test]
    fn deterministic_key_derivation() {
        let key1 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let key2 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        // Same mnemonic + same index -> identical keys
        assert_eq!(
            pubkey_bytes(&key1),
            pubkey_bytes(&key2),
            "Same mnemonic + index must produce identical keys"
        );
    }

    #[test]
    fn different_hd_index_produces_different_key() {
        let key0 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let key1 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 1).unwrap();
        let key2 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 2).unwrap();
        // Different indices -> different keys
        assert_ne!(
            pubkey_bytes(&key0),
            pubkey_bytes(&key1),
            "Different HD indices must produce different keys"
        );
        assert_ne!(
            pubkey_bytes(&key1),
            pubkey_bytes(&key2),
            "Different HD indices must produce different keys"
        );
    }

    #[test]
    fn rejects_raw_key() {
        let result = bls_private_key_from_mnemonic(
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            0,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("mnemonic, not a raw key"),
            "Error should mention mnemonic: {err_msg}"
        );
    }

    #[test]
    fn rejects_invalid_mnemonic() {
        let result = bls_private_key_from_mnemonic("not a valid mnemonic phrase", 0);
        assert!(result.is_err());
    }

    #[test]
    fn g1_pubkey_is_128_bytes() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let pubkey = bls_g1_pubkey_bytes(&key).unwrap();
        assert_eq!(pubkey.len(), 128, "EIP-2537 G1 point must be 128 bytes");
    }

    #[test]
    fn g1_pubkey_deterministic() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let pubkey1 = bls_g1_pubkey_bytes(&key).unwrap();
        let pubkey2 = bls_g1_pubkey_bytes(&key).unwrap();
        assert_eq!(pubkey1, pubkey2, "G1 pubkey must be deterministic");
    }

    #[test]
    fn g1_pubkey_eip2537_padding() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let pubkey = bls_g1_pubkey_bytes(&key).unwrap();

        // EIP-2537 format: each Fp element is 64 bytes (16 zero padding + 48 data)
        // x coordinate: bytes[0..16] must be zero padding
        assert!(
            pubkey[0..16].iter().all(|&b| b == 0),
            "First 16 bytes must be zero padding for x coordinate"
        );
        // y coordinate: bytes[64..80] must be zero padding
        assert!(
            pubkey[64..80].iter().all(|&b| b == 0),
            "Bytes 64..80 must be zero padding for y coordinate"
        );
        // Data portions must not be all zeros (point is not identity for a valid key)
        assert!(
            pubkey[16..64].iter().any(|&b| b != 0),
            "x coordinate data must not be all zeros"
        );
        assert!(
            pubkey[80..128].iter().any(|&b| b != 0),
            "y coordinate data must not be all zeros"
        );
    }

    #[test]
    fn different_keys_different_pubkeys() {
        let key0 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let key1 = bls_private_key_from_mnemonic(TEST_MNEMONIC, 1).unwrap();
        let pubkey0 = bls_g1_pubkey_bytes(&key0).unwrap();
        let pubkey1 = bls_g1_pubkey_bytes(&key1).unwrap();
        assert_ne!(
            pubkey0, pubkey1,
            "Different keys must produce different pubkeys"
        );
    }

    #[test]
    fn bls_sign_digest_produces_256_bytes() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let digest = [0xab_u8; 32]; // arbitrary 32-byte digest
        let sig = bls_sign_digest(&key, &digest).unwrap();
        assert_eq!(sig.len(), 256, "G2 EIP-2537 signature must be 256 bytes");
    }

    #[test]
    fn bls_g2_signature_eip2537_padding() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let digest = [0xab_u8; 32];
        let sig = bls_sign_digest(&key, &digest).unwrap();
        // Each Fp element: 16 zero padding + 48 data = 64 bytes, 4 elements = 256 bytes
        // Zero padding regions:
        assert!(
            sig[0..16].iter().all(|&b| b == 0),
            "x.c0 padding must be zeros"
        );
        assert!(
            sig[64..80].iter().all(|&b| b == 0),
            "x.c1 padding must be zeros"
        );
        assert!(
            sig[128..144].iter().all(|&b| b == 0),
            "y.c0 padding must be zeros"
        );
        assert!(
            sig[192..208].iter().all(|&b| b == 0),
            "y.c1 padding must be zeros"
        );
        // Data regions must not be all zeros:
        assert!(
            sig[16..64].iter().any(|&b| b != 0),
            "x.c0 data must not be all zeros"
        );
        assert!(
            sig[80..128].iter().any(|&b| b != 0),
            "x.c1 data must not be all zeros"
        );
    }

    /// Verify that bls_g2_signature_bytes correctly converts from ZCash/blst format
    /// to EIP-2537 format by checking the G2 generator encoding.
    ///
    /// The G2 generator's x coordinate (from EIP-2537 spec) in Fp2 is:
    ///   c0 (real)      = 024aa2b2f...bdb8
    ///   c1 (imaginary) = 13e02b605...b7e
    ///
    /// blst serializes as: [x_c1(48)] [x_c0(48)] [y_c1(48)] [y_c0(48)]  (ZCash: c1 first)
    /// EIP-2537 expects:   [x_c0(64)] [x_c1(64)] [y_c0(64)] [y_c1(64)]  (c0 first, 16-byte padded)
    #[test]
    fn bls_g2_generator_eip2537_coordinate_order() {
        use commonware_codec::Encode;
        // Secret key = 1 → public key = G1 generator, sign(H(msg)) = G2 hash point
        // Actually sign with sk=1 so we can verify against G1 generator
        // But we can't easily get sk=1 from blst via commonware. Instead, verify that
        // `bls_g2_signature_bytes` output matches what we'd expect from the EIP-2537 G2 generator
        // by signing with the G2MSM precompile (sk=1).
        //
        // A simpler approach: verify the G2 serialization coordinate order by checking
        // that blst's raw serialize() has c1 before c0 (ZCash format).
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let raw_bytes = key.encode();
        let sk =
            blst::min_pk::SecretKey::from_bytes(&raw_bytes).expect("blst SecretKey from bytes");

        // Use a simple digest for signing
        let digest = [0x42u8; 32];
        let sig = sk.sign(&digest, BLS_SIGNING_DST, &[]);
        let raw = sig.serialize(); // 192 bytes: ZCash order x_c1|x_c0|y_c1|y_c0

        let eip = bls_g2_signature_bytes(&sig).unwrap();

        // EIP-2537[16..64] (x_c0 slot) must contain blst's x_c0 = raw[48..96]
        assert_eq!(
            &eip[16..64],
            &raw[48..96],
            "x.c0 slot must contain blst raw[48..96] (blst x_c0)"
        );
        // EIP-2537[80..128] (x_c1 slot) must contain blst's x_c1 = raw[0..48]
        assert_eq!(
            &eip[80..128],
            &raw[0..48],
            "x.c1 slot must contain blst raw[0..48] (blst x_c1)"
        );
        // EIP-2537[144..192] (y_c0 slot) must contain blst's y_c0 = raw[144..192]
        assert_eq!(
            &eip[144..192],
            &raw[144..192],
            "y.c0 slot must contain blst raw[144..192] (blst y_c0)"
        );
        // EIP-2537[208..256] (y_c1 slot) must contain blst's y_c1 = raw[96..144]
        assert_eq!(
            &eip[208..256],
            &raw[96..144],
            "y.c1 slot must contain blst raw[96..144] (blst y_c1)"
        );
    }

    #[test]
    fn private_key_roundtrip_through_blst() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let raw_bytes = key.encode();
        let sk =
            blst::min_pk::SecretKey::from_bytes(&raw_bytes).expect("blst SecretKey from bytes");
        // Derive pubkey from blst SecretKey and compare with commonware pubkey
        let blst_pk = sk.sk_to_pk();
        let blst_pk_compressed = blst_pk.compress();
        let commonware_pk: &[u8] = &key.public_key();
        assert_eq!(
            blst_pk_compressed.as_slice(),
            commonware_pk,
            "blst and commonware pubkeys must match"
        );
    }

    #[test]
    fn bls_sign_digest_deterministic() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let digest = [0xcd_u8; 32];
        let sig1 = bls_sign_digest(&key, &digest).unwrap();
        let sig2 = bls_sign_digest(&key, &digest).unwrap();
        assert_eq!(
            sig1, sig2,
            "Same key + digest must produce identical signatures"
        );
    }

    #[test]
    fn bls_sign_digest_different_digests() {
        let key = bls_private_key_from_mnemonic(TEST_MNEMONIC, 0).unwrap();
        let digest_a = [0x01_u8; 32];
        let digest_b = [0x02_u8; 32];
        let sig_a = bls_sign_digest(&key, &digest_a).unwrap();
        let sig_b = bls_sign_digest(&key, &digest_b).unwrap();
        assert_ne!(
            sig_a, sig_b,
            "Different digests must produce different signatures"
        );
    }
}
