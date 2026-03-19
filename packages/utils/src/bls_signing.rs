//! BLS12-381 key derivation from BIP-39 mnemonic.
//!
//! Derives deterministic BLS private keys using HKDF-SHA256 to incorporate
//! the HD index, then seeds ChaCha20Rng for commonware's PrivateKey::random().
//! This parallels the ed25519 derivation in aggregator/p2p.rs but adds HKDF
//! to safely derive multiple keys from a single mnemonic.

use commonware_cryptography::bls12381;

/// Domain separation label for HKDF info parameter.
/// Prevents accidental collision with other HKDF usages of the same BIP-39 seed.
const HKDF_INFO_PREFIX: &[u8] = b"WAVS-BLS-KEY-v1";

/// Derive a BLS12-381 private key deterministically from a BIP-39 mnemonic and HD index.
pub fn bls_private_key_from_mnemonic(
    _mnemonic: &str,
    _hd_index: u32,
) -> anyhow::Result<bls12381::PrivateKey> {
    todo!("RED phase: not yet implemented")
}

/// Convert a BLS private key's G1 public key to 128-byte EIP-2537 uncompressed format.
pub fn bls_g1_pubkey_bytes(
    _private_key: &bls12381::PrivateKey,
) -> anyhow::Result<[u8; 128]> {
    todo!("RED phase: not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    const TEST_MNEMONIC: &str =
        "test test test test test test test test test test test junk";

    /// Helper to extract compressed pubkey bytes (48 bytes) for comparison.
    fn pubkey_bytes(key: &bls12381::PrivateKey) -> Vec<u8> {
        let pk = key.public_key();
        // Use Deref<Target=[u8]> to get the raw bytes
        let bytes: &[u8] = &*pk;
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
        assert_ne!(pubkey0, pubkey1, "Different keys must produce different pubkeys");
    }
}
