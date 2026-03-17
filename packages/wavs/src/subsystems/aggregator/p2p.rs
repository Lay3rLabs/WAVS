//! P2P Network Layer for WAVS Aggregator
//!
//! This module provides peer-to-peer networking for multi-operator WAVS deployments,
//! enabling operators to share submissions and reach quorum consensus.
//!
//! # Migration Status
//!
//! This module is being migrated from libp2p to commonware-p2p. The Ed25519 identity
//! derivation and commonware runtime scaffold are implemented. The networking layer uses
//! lookup mode (known addresses) for local dev; discovery mode will be added in Plan 03.
//! Broadcast, message routing, and the full P2pHandle API will be implemented in Phase 2.

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
            P2pConfig::Local {
                authorized_peers, ..
            } => authorized_peers,
            P2pConfig::Remote {
                authorized_peers, ..
            } => authorized_peers,
            P2pConfig::Disabled => &[],
        }
    }
}

// ============================================================================
// Network Management (Commonware Runtime + Lookup Mode)
// ============================================================================

use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::authenticated::lookup;
use commonware_p2p::{Address, AddressableManager};
use commonware_runtime::Quota;
use commonware_utils::NZU32;

/// Parse a peer address string of format "<hex_pubkey>@<host>:<port>"
/// into an Ed25519 public key and socket address.
fn parse_peer_address(
    addr: &str,
) -> Result<(ed25519::PublicKey, std::net::SocketAddr), AggregatorError> {
    let parts: Vec<&str> = addr.splitn(2, '@').collect();
    if parts.len() != 2 {
        return Err(AggregatorError::P2p(format!(
            "Invalid peer address format '{}', expected '<hex_pubkey>@<host>:<port>'",
            addr
        )));
    }
    let pubkey_bytes = const_hex::decode(parts[0])
        .map_err(|e| AggregatorError::P2p(format!("Invalid hex pubkey in '{}': {}", addr, e)))?;
    let pubkey = pubkey_from_bytes(&pubkey_bytes).map_err(|e| {
        AggregatorError::P2p(format!("Invalid Ed25519 pubkey in '{}': {}", addr, e))
    })?;
    let socket_addr: std::net::SocketAddr = parts[1]
        .parse()
        .map_err(|e| AggregatorError::P2p(format!("Invalid socket address in '{}': {}", addr, e)))?;
    Ok((pubkey, socket_addr))
}

/// Parse hex-encoded Ed25519 public key strings into PublicKey values.
fn parse_authorized_peers(
    hex_keys: &[String],
) -> Result<Vec<ed25519::PublicKey>, AggregatorError> {
    hex_keys
        .iter()
        .map(|hex| {
            let bytes = const_hex::decode(hex).map_err(|e| {
                AggregatorError::P2p(format!("Invalid hex pubkey '{}': {}", hex, e))
            })?;
            pubkey_from_bytes(&bytes)
                .map_err(|e| AggregatorError::P2p(format!("Invalid Ed25519 pubkey '{}': {}", hex, e)))
        })
        .collect()
}

/// Construct an Ed25519 PublicKey from raw bytes using commonware's codec.
fn pubkey_from_bytes(bytes: &[u8]) -> Result<ed25519::PublicKey, String> {
    use commonware_codec::ReadExt;
    let mut buf = &bytes[..];
    ed25519::PublicKey::read(&mut buf).map_err(|e| format!("{}", e))
}

/// Spawn the commonware P2P runtime on a dedicated OS thread.
///
/// Returns a JoinHandle for the OS thread and accepts an mpsc receiver for P2pCommands.
///
/// CRITICAL: Must use std::thread::spawn (NOT tokio::spawn or spawn_blocking)
/// because commonware's Runner creates its own Tokio runtime internally.
/// Nesting Tokio runtimes causes a panic.
fn spawn_commonware_runtime(
    private_key: ed25519::PrivateKey,
    p2p_config: P2pConfig,
    command_rx: mpsc::UnboundedReceiver<P2pCommand>,
) -> Result<std::thread::JoinHandle<()>, AggregatorError> {
    let handle = std::thread::Builder::new()
        .name("wavs-p2p-commonware".into())
        .spawn(move || {
            use commonware_runtime::{tokio::Config as RuntimeConfig, Runner};
            let runner = commonware_runtime::tokio::Runner::new(
                RuntimeConfig::new()
                    .with_worker_threads(2)
                    .with_max_blocking_threads(4)
                    .with_tcp_nodelay(Some(true)),
            );
            runner.start(|context| async move {
                match p2p_config {
                    P2pConfig::Local {
                        listen_port,
                        ref peer_addresses,
                        ref authorized_peers,
                    } => {
                        run_lookup_network(
                            context,
                            &private_key,
                            listen_port,
                            peer_addresses,
                            authorized_peers,
                            command_rx,
                        )
                        .await;
                    }
                    P2pConfig::Remote { .. } => {
                        // Discovery mode -- implemented in Plan 03
                        tracing::warn!("Discovery mode not yet implemented");
                    }
                    P2pConfig::Disabled => {
                        // Should not reach here; handled before spawn
                    }
                }
            });
        })
        .map_err(|e| AggregatorError::P2p(format!("Failed to spawn P2P thread: {}", e)))?;
    Ok(handle)
}

/// Run a lookup-mode P2P network inside the commonware runtime.
///
/// This function:
/// 1. Creates a lookup::Network with the node's Ed25519 identity
/// 2. Configures the Oracle with authorized peers + own pubkey
/// 3. Registers a single broadcast channel (for Phase 2)
/// 4. Starts the network
/// 5. Runs a bridge loop handling P2pCommands from the WAVS main runtime
///
/// SEC-02: Rate limiting is active via lookup::Config::local() which sets:
/// - allowed_connection_rate_per_peer: Quota::per_second(1)
/// - allowed_handshake_rate_per_ip: Quota::per_second(16)
/// - allowed_handshake_rate_per_subnet: Quota::per_second(128)
/// These are non-zero defaults confirmed from the Config::local() source.
async fn run_lookup_network(
    context: impl commonware_runtime::Spawner
        + commonware_runtime::Clock
        + commonware_runtime::Network
        + commonware_runtime::Metrics
        + commonware_runtime::BufferPooler
        + commonware_runtime::Resolver
        + rand_core::CryptoRngCore,
    private_key: &ed25519::PrivateKey,
    listen_port: u16,
    peer_addresses: &[String],
    authorized_peer_hexes: &[String],
    mut command_rx: mpsc::UnboundedReceiver<P2pCommand>,
) {
    let listen_addr = std::net::SocketAddr::from(([0, 0, 0, 0], listen_port));

    // Create lookup network config with rate limiting active (Config::local defaults).
    // SEC-02 verified: Config::local() sets allowed_connection_rate_per_peer = per_second(1),
    // allowed_handshake_rate_per_ip = per_second(16), allowed_handshake_rate_per_subnet = per_second(128).
    let config = lookup::Config::local(
        private_key.clone(),
        b"wavs-p2p", // namespace for replay protection
        listen_addr,
        65536, // max_message_size (64KB)
    );

    tracing::debug!(
        "P2P lookup config: rate limiting active (connection_rate_per_peer=1/s, handshake_rate_per_ip=16/s, handshake_rate_per_subnet=128/s)"
    );

    let (mut network, mut oracle) =
        lookup::Network::new(context.with_label("p2p_network"), config);

    // Build Oracle peer map: Map<PublicKey, Address>
    // Include own pubkey (implicitly trusted per user decision)
    let own_pubkey = private_key.public_key();

    let mut peer_entries: Vec<(ed25519::PublicKey, Address)> = Vec::new();

    // Add own pubkey to Oracle so other peers can verify us
    peer_entries.push((own_pubkey.clone(), listen_addr.into()));

    // Parse and add known peer addresses
    for addr_str in peer_addresses {
        match parse_peer_address(addr_str) {
            Ok((pubkey, socket_addr)) => {
                peer_entries.push((pubkey, socket_addr.into()));
            }
            Err(e) => {
                tracing::error!("Skipping invalid peer address '{}': {}", addr_str, e);
            }
        }
    }

    // Parse and add authorized peers (these may not have addresses in lookup mode,
    // but they need to be in the Oracle for authorization).
    // NOTE: In lookup mode, peers without addresses cannot be connected to,
    // but authorized_peers without addresses are still recognized if they connect to us.
    for hex_key in authorized_peer_hexes {
        match parse_authorized_peers(&[hex_key.clone()]) {
            Ok(keys) => {
                for key in keys {
                    if !peer_entries.iter().any(|(k, _)| k == &key) {
                        // Add with placeholder address -- they connect to us, we don't need their address
                        peer_entries
                            .push((key, std::net::SocketAddr::from(([0, 0, 0, 0], 0)).into()));
                    }
                }
            }
            Err(e) => {
                tracing::error!("Skipping invalid authorized peer '{}': {}", hex_key, e);
            }
        }
    }

    // Build the ordered Map for Oracle.track()
    // Map requires TryFrom<Vec<(K, V)>> which requires sorted, unique keys
    let peer_map: commonware_utils::ordered::Map<ed25519::PublicKey, Address> =
        commonware_utils::ordered::Map::from_iter_dedup(peer_entries);

    // Register peer set with Oracle at index 0
    oracle.track(0, peer_map).await;

    tracing::info!(
        "P2P lookup network: listening on {}, {} peers configured",
        listen_addr,
        peer_addresses.len()
    );

    // Register a single broadcast channel (channel 0) for Phase 2 use.
    // Must be registered before network.start().
    let (_sender, _receiver) = network.register(
        0u64,
        Quota::per_second(NZU32!(100)),
        1024, // backlog
    );

    // Start the network (consumes self, returns a handle)
    let _net_handle = network.start();

    tracing::info!(
        "P2P network started (peer_id: {})",
        const_hex::encode(own_pubkey.as_ref())
    );

    // Bridge loop: handle P2pCommands from WAVS main runtime.
    // Phase 1 only handles GetStatus; Publish/Subscribe/Unsubscribe are Phase 2.
    loop {
        match command_rx.recv().await {
            Some(P2pCommand::GetStatus { response_tx }) => {
                let status = P2pStatus {
                    enabled: true,
                    local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
                    listen_addresses: vec![listen_addr.to_string()],
                    external_addresses: vec![],
                    connected_peers: 0, // Phase 2 fills this from network state
                    peer_ids: vec![],   // Phase 2 fills this
                    subscribed_topics: vec![], // Phase 2 fills this
                    topic_peer_counts: Default::default(), // Phase 2 fills this
                };
                let _ = response_tx.send(status);
            }
            Some(_) => {
                // Publish, Subscribe, Unsubscribe handled in Phase 2
                tracing::debug!("P2pCommand not yet implemented in Phase 1 Plan 02");
            }
            None => {
                // Channel closed -- shutdown
                tracing::info!("P2P command channel closed, shutting down network");
                // Signal the commonware runtime to stop
                context.stop(0, None).await.ok();
                break;
            }
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
        signing_mnemonic: Option<&str>,
        _aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    ) -> Result<Option<Self>, AggregatorError> {
        if matches!(p2p_config, P2pConfig::Disabled) {
            tracing::info!("P2P networking is disabled");
            return Ok(None);
        }

        let mnemonic = signing_mnemonic.ok_or_else(|| {
            AggregatorError::P2p("signing_mnemonic required for P2P networking".into())
        })?;
        let private_key = ed25519_signer_from_mnemonic(mnemonic)?;

        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Spawn commonware runtime on dedicated OS thread
        let _thread_handle = spawn_commonware_runtime(private_key, p2p_config, command_rx)?;

        // TODO: Store thread_handle for clean shutdown in Phase 2

        Ok(Some(P2pHandle { command_tx }))
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
            .map_err(|e| {
                AggregatorError::P2p(format!("Failed to send unsubscribe command: {}", e))
            })
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
