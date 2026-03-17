//! P2P Network Layer for WAVS Aggregator
//!
//! This module provides peer-to-peer networking for multi-operator WAVS deployments,
//! enabling operators to share submissions and reach quorum consensus.
//!
//! # Migration Status
//!
//! This module is being migrated from libp2p to commonware-p2p. Currently only the
//! Ed25519 identity derivation is implemented. The networking layer (lookup/discovery
//! modes, broadcast, event loop) will be implemented in Plans 02 and 03.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utils::context::AppContext;
use wavs_types::{P2pStatus, ServiceId, Submission};

use super::{error::AggregatorError, AggregatorCommand};

// ============================================================================
// P2P Configuration
// ============================================================================

/// P2P networking configuration.
///
/// - `Disabled`: No P2P networking (single-operator setups).
/// - `Local`: Lookup mode with known peer addresses (local dev / testing).
/// - `Remote`: Discovery mode with bootstrapper nodes (production).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum P2pConfig {
    #[default]
    Disabled,
    /// Local development -- lookup mode with known peer addresses.
    Local {
        /// Port to listen on for P2P connections.
        listen_port: u16,
        /// Known peer addresses for lookup mode: ["<hex_pubkey>@<host>:<port>", ...]
        #[serde(default)]
        peer_addresses: Vec<String>,
        /// Authorized peer Ed25519 public keys (hex-encoded).
        /// The local node's own pubkey is implicitly trusted.
        #[serde(default)]
        authorized_peers: Vec<String>,
    },
    /// Remote / production -- discovery mode with bootstrapper nodes.
    Remote {
        /// Port to listen on for P2P connections.
        listen_port: u16,
        /// Bootstrapper addresses: ["<hex_pubkey>@<host>:<port>", ...]
        #[serde(default)]
        bootstrappers: Vec<String>,
        /// Authorized peer Ed25519 public keys (hex-encoded).
        /// The local node's own pubkey is implicitly trusted.
        #[serde(default)]
        authorized_peers: Vec<String>,
    },
}

impl P2pConfig {
    /// Returns the listen port, or None if disabled.
    pub fn listen_port(&self) -> Option<u16> {
        match self {
            P2pConfig::Local { listen_port, .. } => Some(*listen_port),
            P2pConfig::Remote { listen_port, .. } => Some(*listen_port),
            P2pConfig::Disabled => None,
        }
    }

    /// Returns the authorized peer hex pubkeys.
    pub fn authorized_peers(&self) -> &[String] {
        match self {
            P2pConfig::Local { authorized_peers, .. } => authorized_peers,
            P2pConfig::Remote { authorized_peers, .. } => authorized_peers,
            P2pConfig::Disabled => &[],
        }
    }
}

// ============================================================================
// P2P Handle and Commands
// ============================================================================

/// Commands that can be sent to the P2P network
enum P2pCommand {
    /// Publish a submission to the network
    Publish {
        service_id: ServiceId,
        submission: Box<Submission>,
    },
    /// Subscribe to a service's topic
    Subscribe { service_id: ServiceId },
    /// Unsubscribe from a service's topic
    Unsubscribe { service_id: ServiceId },
    /// Get the current P2P status
    GetStatus {
        response_tx: tokio::sync::oneshot::Sender<P2pStatus>,
    },
}

/// Handle to the P2P network that can be cloned and shared
#[derive(Clone)]
pub struct P2pHandle {
    command_tx: mpsc::UnboundedSender<P2pCommand>,
}

impl P2pHandle {
    /// Create a new P2P handle, spawning the network event loop.
    ///
    /// Returns None if P2P is disabled.
    ///
    /// If `signing_mnemonic` is provided, the P2P identity will be derived from it
    /// as a deterministic Ed25519 keypair, ensuring a consistent peer ID across restarts.
    pub async fn new(
        _ctx: AppContext,
        p2p_config: P2pConfig,
        _signing_mnemonic: Option<&str>,
        _aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    ) -> Result<Option<Self>, AggregatorError> {
        if matches!(p2p_config, P2pConfig::Disabled) {
            tracing::info!("P2P networking is disabled");
            Ok(None)
        } else {
            // P2P networking is not yet reimplemented with commonware.
            // Identity derivation is available via ed25519_signer_from_mnemonic().
            // Full networking (lookup/discovery modes) will be implemented in Plans 02 and 03.
            Err(AggregatorError::P2p(
                "P2P not yet reimplemented with commonware".into(),
            ))
        }
    }

    /// Publish a submission to the P2P network
    pub fn publish(&self, submission: &Submission) -> Result<(), AggregatorError> {
        let service_id = submission.service_id().clone();
        self.command_tx
            .send(P2pCommand::Publish {
                service_id,
                submission: Box::new(submission.clone()),
            })
            .map_err(|e| AggregatorError::P2p(format!("Failed to send publish command: {}", e)))
    }

    /// Subscribe to a service's P2P topic
    pub fn subscribe(&self, service_id: &ServiceId) -> Result<(), AggregatorError> {
        self.command_tx
            .send(P2pCommand::Subscribe {
                service_id: service_id.clone(),
            })
            .map_err(|e| AggregatorError::P2p(format!("Failed to send subscribe command: {}", e)))
    }

    /// Unsubscribe from a service's P2P topic
    pub fn unsubscribe(&self, service_id: &ServiceId) -> Result<(), AggregatorError> {
        self.command_tx
            .send(P2pCommand::Unsubscribe {
                service_id: service_id.clone(),
            })
            .map_err(|e| AggregatorError::P2p(format!("Failed to send unsubscribe command: {}", e)))
    }

    /// Get the current P2P network status
    pub async fn get_status(&self) -> Result<P2pStatus, AggregatorError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(P2pCommand::GetStatus { response_tx })
            .map_err(|e| {
                AggregatorError::P2p(format!("Failed to send get_status command: {}", e))
            })?;

        response_rx
            .await
            .map_err(|e| AggregatorError::P2p(format!("Failed to receive P2P status: {}", e)))
    }
}

// ============================================================================
// Identity Management (Ed25519 via commonware-cryptography)
// ============================================================================

use commonware_cryptography::{ed25519, Signer};
use commonware_math::algebra::Random;
use rand_chacha::ChaCha20Rng;

/// Derive a deterministic Ed25519 identity from a BIP-39 mnemonic.
///
/// Uses ChaCha20Rng seeded from the first 32 bytes of the BIP-39 seed
/// (PBKDF2-stretched, empty passphrase) for full 256-bit entropy.
///
/// Replaces the previous `keypair_from_mnemonic()` which derived a secp256k1
/// keypair at HD path m/44'/60'/0'/0/0.
pub fn ed25519_signer_from_mnemonic(mnemonic: &str) -> Result<ed25519::PrivateKey, AggregatorError> {
    let mnemonic = bip39::Mnemonic::parse(mnemonic)
        .map_err(|e| AggregatorError::P2p(format!("Invalid mnemonic: {}", e)))?;

    // BIP-39 seed: 64 bytes, PBKDF2-stretched with empty passphrase
    let seed = mnemonic.to_seed("");

    // Use first 32 bytes as ChaCha20Rng seed (full 256-bit entropy)
    let rng_seed: [u8; 32] = seed[..32]
        .try_into()
        .map_err(|_| AggregatorError::P2p("BIP-39 seed too short for RNG seed".into()))?;

    use rand_chacha::rand_core::SeedableRng;
    let mut rng = ChaCha20Rng::from_seed(rng_seed);
    Ok(ed25519::PrivateKey::random(&mut rng))
}

/// Get the P2P public key (hex-encoded) that would be derived from a given mnemonic.
/// Useful for determining the peer identity before starting the node.
pub fn pubkey_from_mnemonic(mnemonic: &str) -> Result<String, AggregatorError> {
    let private_key = ed25519_signer_from_mnemonic(mnemonic)?;
    Ok(const_hex::encode(private_key.public_key().as_ref()))
}
