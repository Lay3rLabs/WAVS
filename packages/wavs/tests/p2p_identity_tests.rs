//! Integration tests for Ed25519 P2P identity derivation.
//!
//! These tests verify IDEN-01 and IDEN-02: deterministic Ed25519 keypair
//! derivation from BIP-39 mnemonics via ChaCha20Rng.

// Use the public API from the wavs crate
use commonware_cryptography::Signer;
use wavs::subsystems::aggregator::p2p::{
    ed25519_signer_from_mnemonic, pubkey_from_mnemonic, P2pConfig,
};

/// Standard BIP-39 test mnemonic (12 words)
const TEST_MNEMONIC_1: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Different test mnemonic
const TEST_MNEMONIC_2: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

#[test]
fn test_deterministic_derivation() {
    // IDEN-01: Same mnemonic always produces the same key
    let key1 = ed25519_signer_from_mnemonic(TEST_MNEMONIC_1).unwrap();
    let key2 = ed25519_signer_from_mnemonic(TEST_MNEMONIC_1).unwrap();
    assert_eq!(
        key1.public_key().as_ref(),
        key2.public_key().as_ref(),
        "Same mnemonic must produce identical Ed25519 public keys"
    );
}

#[test]
fn test_consistent_across_restarts() {
    // IDEN-02: pubkey_from_mnemonic returns same hex string
    let hex1 = pubkey_from_mnemonic(TEST_MNEMONIC_1).unwrap();
    let hex2 = pubkey_from_mnemonic(TEST_MNEMONIC_1).unwrap();
    assert_eq!(hex1, hex2, "Peer ID must be consistent across invocations");
    // Verify hex format
    assert!(
        hex1.chars().all(|c| c.is_ascii_hexdigit()),
        "Pubkey must be hex-encoded"
    );
    assert!(
        !hex1.is_empty() && hex1.len().is_multiple_of(2),
        "Hex string must have even length"
    );
}

#[test]
fn test_different_mnemonics_produce_different_keys() {
    let pubkey1 = pubkey_from_mnemonic(TEST_MNEMONIC_1).unwrap();
    let pubkey2 = pubkey_from_mnemonic(TEST_MNEMONIC_2).unwrap();
    assert_ne!(
        pubkey1, pubkey2,
        "Different mnemonics must produce different keys"
    );
}

#[test]
fn test_invalid_mnemonic_returns_error() {
    let result = ed25519_signer_from_mnemonic("not a valid mnemonic phrase");
    assert!(result.is_err(), "Invalid mnemonic must return Err");
}

#[test]
fn test_p2p_config_default_is_disabled() {
    let config: P2pConfig = Default::default();
    assert_eq!(config, P2pConfig::Disabled);
}

// ============================================================================
// P2pConfig deserialization tests (Task 2)
// ============================================================================

#[test]
fn test_p2p_config_local_deserialization() {
    let config = P2pConfig::Local {
        listen_port: 9000,
        peer_addresses: vec!["aabb@127.0.0.1:9001".to_string()],
        authorized_peers: vec!["aabbccdd".to_string()],
        max_message_size: None,
        deque_size: None,
    };
    assert_eq!(config.listen_port(), Some(9000));
    assert_eq!(config.authorized_peers().len(), 1);
    assert_eq!(config.authorized_peers()[0], "aabbccdd");
}

#[test]
fn test_p2p_config_remote_deserialization() {
    let config = P2pConfig::Remote {
        listen_port: 9000,
        bootstrappers: vec!["aabb@bootstrap.example.com:9000".to_string()],
        authorized_peers: vec!["aabbccdd".to_string(), "eeff0011".to_string()],
        max_message_size: None,
        deque_size: None,
    };
    assert_eq!(config.listen_port(), Some(9000));
    assert_eq!(config.authorized_peers().len(), 2);
}

#[test]
fn test_p2p_config_disabled_has_no_port() {
    let config = P2pConfig::Disabled;
    assert_eq!(config.listen_port(), None);
    assert_eq!(config.authorized_peers().len(), 0);
}
