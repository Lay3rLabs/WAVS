//! P2P Network Layer for WAVS Aggregator
//!
//! This module provides peer-to-peer networking for multi-operator WAVS deployments,
//! enabling operators to share submissions and reach quorum consensus.
//!
//! # Migration Status
//!
//! This module is being migrated from libp2p to commonware-p2p. The Ed25519 identity
//! derivation, commonware runtime scaffold, lookup mode (known addresses for local dev),
//! and discovery mode (bootstrapper-based for production) are implemented.
//! Broadcast, message routing, and the full P2pHandle API will be implemented in Phase 2.

use std::collections::{HashSet, VecDeque};

use commonware_codec::{EncodeSize, Error as CodecError, RangeCfg, Read as CodecRead, ReadRangeExt, Write as CodecWrite};
use commonware_cryptography::{sha256, Digestible, Hasher, Sha256};
use commonware_runtime::{Buf, BufMut};
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
// P2P Message Envelope (Codec + Digestible)
// ============================================================================

/// P2P message envelope for broadcast.
/// Wraps a ServiceId (32 bytes) and serialized Submission for transmission
/// over commonware-broadcast. The Digestible impl produces a SHA-256 digest
/// used by the broadcast Engine for deduplication (BCAST-02).
#[derive(Clone, Debug)]
pub struct P2pMessage {
    /// Service ID bytes (raw 32-byte hash from ServiceId::inner())
    pub service_id_bytes: [u8; 32],
    /// JSON-serialized Submission payload
    pub payload: Vec<u8>,
}

impl P2pMessage {
    /// Create a P2pMessage from a ServiceId and Submission.
    ///
    /// The Submission is JSON-serialized into the payload field.
    pub fn from_submission(
        service_id: &ServiceId,
        submission: &Submission,
    ) -> Result<Self, AggregatorError> {
        let payload = serde_json::to_vec(submission)
            .map_err(|e| AggregatorError::P2p(format!("Failed to serialize submission: {}", e)))?;
        Ok(Self {
            service_id_bytes: service_id.inner(),
            payload,
        })
    }

    /// Deserialize this P2pMessage back into a (ServiceId, Submission) pair.
    pub fn to_submission(&self) -> Result<(ServiceId, Submission), AggregatorError> {
        let service_id = ServiceId::from(self.service_id_bytes);
        let submission: Submission = serde_json::from_slice(&self.payload).map_err(|e| {
            AggregatorError::P2p(format!("Failed to deserialize submission: {}", e))
        })?;
        Ok((service_id, submission))
    }
}

impl Digestible for P2pMessage {
    type Digest = sha256::Digest;

    fn digest(&self) -> sha256::Digest {
        let mut data = Vec::with_capacity(32 + self.payload.len());
        data.extend_from_slice(&self.service_id_bytes);
        data.extend_from_slice(&self.payload);
        Sha256::hash(&data)
    }
}

impl CodecWrite for P2pMessage {
    fn write(&self, buf: &mut impl BufMut) {
        // Write service_id_bytes as fixed 32 bytes (no length prefix)
        buf.put_slice(&self.service_id_bytes);
        // Write payload as Vec<u8> (length-prefixed via commonware codec)
        self.payload.write(buf);
    }
}

impl EncodeSize for P2pMessage {
    fn encode_size(&self) -> usize {
        // Fixed 32 bytes for service_id + Vec<u8> encode size for payload
        32 + self.payload.encode_size()
    }
}

impl CodecRead for P2pMessage {
    /// Cfg is `(RangeCfg<usize>, ())` to match Vec<u8>'s Cfg pattern and allow
    /// use of ReadRangeExt::read_range for ergonomic deserialization.
    type Cfg = (RangeCfg<usize>, ());

    fn read_cfg(buf: &mut impl Buf, (range, _): &Self::Cfg) -> Result<Self, CodecError> {
        // Read fixed 32 bytes for service_id_bytes
        if buf.remaining() < 32 {
            return Err(CodecError::EndOfBuffer);
        }
        let mut service_id_bytes = [0u8; 32];
        buf.copy_to_slice(&mut service_id_bytes);
        // Read payload as Vec<u8> using range config for length validation
        let payload = <Vec<u8>>::read_range(buf, range.clone())?;
        Ok(Self {
            service_id_bytes,
            payload,
        })
    }
}

// ============================================================================
// Service Routing (Application-Level Filtering)
// ============================================================================

/// Application-level message filter for per-service isolation (BCAST-05).
///
/// All messages arrive on a single broadcast channel. The ServiceRouter
/// determines which messages to forward to the Aggregator based on
/// which services this node is subscribed to.
pub(crate) struct ServiceRouter {
    subscribed_services: HashSet<[u8; 32]>,
}

impl ServiceRouter {
    pub fn new() -> Self {
        Self {
            subscribed_services: HashSet::new(),
        }
    }

    pub fn subscribe(&mut self, service_id: &ServiceId) {
        self.subscribed_services.insert(service_id.inner());
    }

    pub fn unsubscribe(&mut self, service_id: &ServiceId) {
        self.subscribed_services.remove(&service_id.inner());
    }

    /// Check whether an inbound P2pMessage is for a subscribed service.
    pub fn should_accept(&self, msg: &P2pMessage) -> bool {
        self.subscribed_services.contains(&msg.service_id_bytes)
    }

    /// Return hex-encoded list of subscribed service IDs for status reporting.
    pub fn subscribed_topics(&self) -> Vec<String> {
        self.subscribed_services
            .iter()
            .map(|s| const_hex::encode(s))
            .collect()
    }
}

// ============================================================================
// Retry Queue for Failed Publishes
// ============================================================================

const MAX_RETRY_QUEUE_SIZE: usize = 64;

/// Bounded queue for messages that failed to broadcast (no connected peers).
/// Messages are retried when the next successful broadcast proves peers are available (BCAST-04).
pub(crate) struct RetryQueue {
    queue: VecDeque<P2pMessage>,
}

impl RetryQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(MAX_RETRY_QUEUE_SIZE),
        }
    }

    /// Add a message to the retry queue. If full, drops the oldest message.
    pub fn push(&mut self, msg: P2pMessage) {
        if self.queue.len() >= MAX_RETRY_QUEUE_SIZE {
            self.queue.pop_front();
            tracing::warn!("Retry queue full, dropping oldest message");
        }
        self.queue.push_back(msg);
    }

    /// Drain all queued messages for retry. Returns them in FIFO order.
    pub fn drain_all(&mut self) -> Vec<P2pMessage> {
        self.queue.drain(..).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ============================================================================
// Network Management (Commonware Runtime + Lookup Mode)
// ============================================================================

use commonware_cryptography::{ed25519, Signer};
use commonware_p2p::authenticated::{discovery, lookup};
use commonware_p2p::{Address, AddressableManager, Blocker, Ingress, Manager};
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

/// Parse a bootstrapper address string of format "<hex_pubkey>@<host>:<port>"
/// into a Bootstrapper tuple (PublicKey, Ingress).
///
/// Bootstrappers in discovery mode need their public key and a dialable address.
fn parse_bootstrapper(
    addr: &str,
) -> Result<(ed25519::PublicKey, Ingress), AggregatorError> {
    let (pubkey, socket_addr) = parse_peer_address(addr)?;
    Ok((pubkey, Ingress::from(socket_addr)))
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
                    P2pConfig::Remote {
                        listen_port,
                        ref bootstrappers,
                        ref authorized_peers,
                    } => {
                        run_discovery_network(
                            context,
                            &private_key,
                            listen_port,
                            bootstrappers,
                            authorized_peers,
                            command_rx,
                        )
                        .await;
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
            Some(P2pCommand::BlockPeer { pubkey_hex }) => {
                match parse_authorized_peers(&[pubkey_hex.clone()]) {
                    Ok(keys) if !keys.is_empty() => {
                        oracle.block(keys[0].clone()).await;
                        tracing::info!("Blocked peer: {}", pubkey_hex);
                    }
                    _ => {
                        tracing::error!("Failed to parse pubkey for blocking: {}", pubkey_hex);
                    }
                }
            }
            Some(_) => {
                // Publish, Subscribe, Unsubscribe handled in Phase 2
                tracing::debug!("P2pCommand not yet implemented in Phase 1");
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

/// Run a discovery-mode P2P network inside the commonware runtime.
///
/// This function:
/// 1. Creates a discovery::Network with the node's Ed25519 identity
/// 2. Configures the Oracle with authorized peers as a Set<PublicKey>
/// 3. Registers a single broadcast channel (for Phase 2)
/// 4. Starts the network
/// 5. Runs a bridge loop handling P2pCommands from the WAVS main runtime
///
/// Discovery mode uses bootstrapper nodes for peer discovery (production).
/// Addresses are discovered dynamically through bootstrappers (no upfront addresses needed).
///
/// NET-01: Discovery-based peer discovery with bootstrappers
/// NET-04: Automatic reconnection (built-in to discovery::Network via dial_frequency)
async fn run_discovery_network(
    context: impl commonware_runtime::Spawner
        + commonware_runtime::Clock
        + commonware_runtime::Network
        + commonware_runtime::Metrics
        + commonware_runtime::BufferPooler
        + commonware_runtime::Resolver
        + rand_core::CryptoRngCore,
    private_key: &ed25519::PrivateKey,
    listen_port: u16,
    bootstrapper_strs: &[String],
    authorized_peer_hexes: &[String],
    mut command_rx: mpsc::UnboundedReceiver<P2pCommand>,
) {
    let listen_addr = std::net::SocketAddr::from(([0, 0, 0, 0], listen_port));
    let own_pubkey = private_key.public_key();

    // Parse bootstrappers
    let mut bootstrappers = Vec::new();
    for bs_str in bootstrapper_strs {
        match parse_bootstrapper(bs_str) {
            Ok(bootstrapper) => bootstrappers.push(bootstrapper),
            Err(e) => {
                tracing::error!("Skipping invalid bootstrapper '{}': {}", bs_str, e);
            }
        }
    }

    if bootstrappers.is_empty() {
        tracing::warn!(
            "No valid bootstrappers configured -- this node will act as a bootstrapper itself"
        );
    }

    // Create discovery network config using Config::local() for allow_private_ips=true
    // (needed for localhost testing). Production deployments should use Config::recommended().
    // Use the node's listen_addr as its dialable address (assumes public IP or port-forwarded).
    let config = discovery::Config::local(
        private_key.clone(),
        b"wavs-p2p", // namespace for replay protection
        listen_addr,
        listen_addr, // dialable_addr -- same as listen for now
        bootstrappers,
        65536, // max_message_size (64KB)
    );

    let (mut network, mut oracle) =
        discovery::Network::new(context.with_label("p2p_discovery"), config);

    // Build Oracle peer set: Set<PublicKey> (commonware_utils::ordered::Set)
    // Discovery Oracle takes Set (not Map like lookup) -- addresses are discovered dynamically.
    let mut peer_keys: Vec<ed25519::PublicKey> = Vec::new();

    // Include own pubkey (implicitly trusted per user decision)
    peer_keys.push(own_pubkey.clone());

    // Add authorized peers
    match parse_authorized_peers(authorized_peer_hexes) {
        Ok(keys) => {
            for key in keys {
                peer_keys.push(key);
            }
        }
        Err(e) => {
            tracing::error!("Failed to parse authorized peers: {}", e);
        }
    }

    // Build ordered Set from peer keys (dedup handles any duplicates)
    let peer_set = commonware_utils::ordered::Set::from_iter_dedup(peer_keys);

    // Register peer set with Oracle at index 0
    oracle.track(0, peer_set).await;

    tracing::info!(
        "P2P discovery network: listening on {}, {} bootstrappers, {} authorized peers",
        listen_addr,
        bootstrapper_strs.len(),
        authorized_peer_hexes.len()
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
        "P2P discovery network started (peer_id: {})",
        const_hex::encode(own_pubkey.as_ref())
    );

    // Bridge loop: handle P2pCommands from WAVS main runtime.
    // Same pattern as lookup mode.
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
            Some(P2pCommand::BlockPeer { pubkey_hex }) => {
                match parse_authorized_peers(&[pubkey_hex.clone()]) {
                    Ok(keys) if !keys.is_empty() => {
                        oracle.block(keys[0].clone()).await;
                        tracing::info!("Blocked peer: {}", pubkey_hex);
                    }
                    _ => {
                        tracing::error!("Failed to parse pubkey for blocking: {}", pubkey_hex);
                    }
                }
            }
            Some(_) => {
                // Publish, Subscribe, Unsubscribe handled in Phase 2
                tracing::debug!("P2pCommand not yet implemented in Phase 1");
            }
            None => {
                // Channel closed -- shutdown
                tracing::info!("P2P command channel closed, shutting down discovery network");
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
    /// Block a peer by their Ed25519 public key (hex-encoded).
    /// The peer will be disconnected and prevented from reconnecting.
    BlockPeer { pubkey_hex: String },
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

    /// Block a misbehaving peer by their Ed25519 public key (hex-encoded).
    /// The peer will be disconnected and prevented from reconnecting.
    pub fn block_peer(&self, pubkey_hex: &str) -> Result<(), AggregatorError> {
        self.command_tx
            .send(P2pCommand::BlockPeer {
                pubkey_hex: pubkey_hex.to_string(),
            })
            .map_err(|e| AggregatorError::P2p(format!("Failed to send block_peer command: {}", e)))
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

// ============================================================================
// Tests: P2P Broadcast Types (Wave 0 stubs)
// ============================================================================

#[cfg(test)]
mod p2p_broadcast_tests {
    use super::*;
    use commonware_codec::{Encode, ReadRangeExt};
    use commonware_cryptography::Digestible;
    use wavs_types::{
        Envelope, EventId, SignatureKind, Trigger, TriggerAction, TriggerConfig, WasmResponse,
        WavsSignature, WorkflowId,
    };

    /// Create a minimal mock Submission for testing.
    fn mock_submission(service_id: &ServiceId) -> Submission {
        let trigger_action = TriggerAction {
            config: TriggerConfig {
                service_id: service_id.clone(),
                workflow_id: WorkflowId::new("test-workflow").unwrap(),
                trigger: Trigger::Manual,
            },
            data: wavs_types::TriggerData::default(),
        };
        let operator_response = WasmResponse {
            payload: b"test-payload".to_vec(),
            event_id_salt: None,
            ordering: None,
        };
        let event_id = EventId::from([1u8; 20]);
        let envelope = Envelope {
            payload: alloy_primitives::Bytes::from_static(&[1, 2, 3]),
            eventId: alloy_primitives::FixedBytes([1; 20]),
            ordering: alloy_primitives::FixedBytes([0; 12]),
        };
        let envelope_signature = WavsSignature {
            data: vec![0u8; 65],
            kind: SignatureKind::evm_default(),
        };
        Submission {
            trigger_action,
            operator_response,
            event_id,
            envelope,
            envelope_signature,
        }
    }

    // ---- P2pMessage tests ----

    #[test]
    fn test_p2p_message_from_submission() {
        // P2pMessage::from_submission() creates message with correct service_id_bytes and payload
        let service_id = ServiceId::hash(b"test-service-a");
        let submission = mock_submission(&service_id);

        let msg = P2pMessage::from_submission(&service_id, &submission).unwrap();

        // service_id_bytes should match the ServiceId's inner bytes
        assert_eq!(msg.service_id_bytes, service_id.inner());

        // payload should be valid JSON that deserializes back to Submission
        let deserialized: Submission = serde_json::from_slice(&msg.payload).unwrap();
        assert_eq!(
            deserialized.service_id(),
            submission.service_id(),
            "Deserialized submission should have the same service_id"
        );
    }

    #[test]
    fn test_p2p_message_codec_roundtrip() {
        // P2pMessage round-trip encode/decode via Write then Read produces identical fields
        let msg = P2pMessage {
            service_id_bytes: [42u8; 32],
            payload: b"hello broadcast world".to_vec(),
        };

        // Encode using the Encode trait (Write + EncodeSize)
        let encoded = msg.encode();

        // Decode using Read with a permissive range config
        let decoded =
            P2pMessage::read_range(&mut encoded.as_ref(), 0..=65536).unwrap();

        assert_eq!(msg.service_id_bytes, decoded.service_id_bytes);
        assert_eq!(msg.payload, decoded.payload);
    }

    #[test]
    fn test_p2p_message_digest_determinism() {
        // Two identical P2pMessages produce the same digest;
        // different messages produce different digests (BCAST-02)
        let msg_a = P2pMessage {
            service_id_bytes: [1u8; 32],
            payload: b"same-payload".to_vec(),
        };
        let msg_b = P2pMessage {
            service_id_bytes: [1u8; 32],
            payload: b"same-payload".to_vec(),
        };
        let msg_c = P2pMessage {
            service_id_bytes: [2u8; 32],
            payload: b"different-payload".to_vec(),
        };

        let digest_a = msg_a.digest();
        let digest_b = msg_b.digest();
        let digest_c = msg_c.digest();

        // Identical messages produce the same digest
        assert_eq!(digest_a, digest_b, "Identical messages must produce same digest");
        // Different messages produce different digests
        assert_ne!(digest_a, digest_c, "Different messages must produce different digests");
    }

    #[test]
    fn test_p2p_message_to_submission_roundtrip() {
        // P2pMessage::to_submission() deserializes back to original (ServiceId, Submission) pair
        let service_id = ServiceId::hash(b"roundtrip-service");
        let original_submission = mock_submission(&service_id);

        let msg = P2pMessage::from_submission(&service_id, &original_submission).unwrap();
        let (recovered_id, recovered_submission) = msg.to_submission().unwrap();

        assert_eq!(recovered_id.inner(), service_id.inner());
        assert_eq!(
            recovered_submission.service_id().inner(),
            original_submission.service_id().inner()
        );
        assert_eq!(
            recovered_submission.operator_response.payload,
            original_submission.operator_response.payload
        );
    }

    // ---- ServiceRouter tests ----

    #[test]
    fn test_service_router_empty_rejects_all() {
        // ServiceRouter::new() creates empty router, should_accept returns false for any message
        let router = ServiceRouter::new();
        let msg = P2pMessage {
            service_id_bytes: [1u8; 32],
            payload: vec![],
        };
        assert!(
            !router.should_accept(&msg),
            "Empty router should reject all messages"
        );
    }

    #[test]
    fn test_service_router_subscribe_accept() {
        // After subscribe(service_id_a), should_accept returns true for matching, false for non-matching
        let service_id_a = ServiceId::hash(b"test-service-a");
        let service_id_b = ServiceId::hash(b"test-service-b");

        let mut router = ServiceRouter::new();
        router.subscribe(&service_id_a);

        let msg_a = P2pMessage {
            service_id_bytes: service_id_a.inner(),
            payload: vec![],
        };
        let msg_b = P2pMessage {
            service_id_bytes: service_id_b.inner(),
            payload: vec![],
        };

        assert!(
            router.should_accept(&msg_a),
            "Should accept messages for subscribed service"
        );
        assert!(
            !router.should_accept(&msg_b),
            "Should reject messages for unsubscribed service"
        );
    }

    #[test]
    fn test_service_router_unsubscribe() {
        // After unsubscribe(service_id_a), should_accept returns false again
        let service_id_a = ServiceId::hash(b"test-service-a");

        let mut router = ServiceRouter::new();
        router.subscribe(&service_id_a);

        let msg_a = P2pMessage {
            service_id_bytes: service_id_a.inner(),
            payload: vec![],
        };

        assert!(router.should_accept(&msg_a), "Should accept after subscribe");
        router.unsubscribe(&service_id_a);
        assert!(
            !router.should_accept(&msg_a),
            "Should reject after unsubscribe"
        );
    }

    #[test]
    fn test_service_router_subscribed_topics() {
        // subscribed_topics() returns hex-encoded list of subscribed service IDs
        let service_id_a = ServiceId::hash(b"test-service-a");

        let mut router = ServiceRouter::new();
        assert!(router.subscribed_topics().is_empty());

        router.subscribe(&service_id_a);
        let topics = router.subscribed_topics();
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0], const_hex::encode(service_id_a.inner()));
    }

    // ---- RetryQueue tests ----

    #[test]
    fn test_retry_queue_empty() {
        // RetryQueue::new() creates empty queue, is_empty() returns true
        let queue = RetryQueue::new();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_retry_queue_push_drain_fifo() {
        // push() adds messages, drain_all() returns them in FIFO order
        let mut queue = RetryQueue::new();

        let msg_a = P2pMessage {
            service_id_bytes: [1u8; 32],
            payload: b"first".to_vec(),
        };
        let msg_b = P2pMessage {
            service_id_bytes: [2u8; 32],
            payload: b"second".to_vec(),
        };

        queue.push(msg_a);
        queue.push(msg_b);
        assert!(!queue.is_empty());

        let drained = queue.drain_all();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].payload, b"first");
        assert_eq!(drained[1].payload, b"second");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_retry_queue_overflow_drops_oldest() {
        // When queue is full (64 items), push() drops oldest message
        let mut queue = RetryQueue::new();

        // Fill to capacity (64)
        for i in 0u8..64 {
            queue.push(P2pMessage {
                service_id_bytes: [i; 32],
                payload: vec![i],
            });
        }
        assert_eq!(queue.drain_all().len(), 64);

        // Refill and then overflow
        for i in 0u8..64 {
            queue.push(P2pMessage {
                service_id_bytes: [i; 32],
                payload: vec![i],
            });
        }
        // Push one more -- oldest (i=0) should be dropped
        queue.push(P2pMessage {
            service_id_bytes: [99u8; 32],
            payload: vec![99],
        });

        let drained = queue.drain_all();
        assert_eq!(drained.len(), 64, "Queue should still hold 64 items");
        // First item should be i=1 (i=0 was dropped)
        assert_eq!(drained[0].payload, vec![1]);
        // Last item should be the overflow item
        assert_eq!(drained[63].payload, vec![99]);
    }

    #[test]
    fn test_retry_queue_drain_empty() {
        // drain_all() on empty queue returns empty Vec
        let mut queue = RetryQueue::new();
        let drained = queue.drain_all();
        assert!(drained.is_empty());
    }
}
