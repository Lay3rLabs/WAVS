//! P2P Network Layer for WAVS Aggregator
//!
//! This module provides peer-to-peer networking for multi-operator WAVS deployments,
//! enabling operators to share submissions and reach quorum consensus.
//!
//! # Architecture
//!
//! Uses commonware-p2p for authenticated peer networking with two modes:
//! - **Lookup mode** (`P2pConfig::Local`): Known peer addresses for local dev/testing
//! - **Discovery mode** (`P2pConfig::Remote`): Bootstrapper-based peer discovery for production
//!
//! ## Broadcast Architecture (Two-Channel)
//!
//! Each network mode registers **two P2P channels**:
//! - **Channel 0**: Consumed by the broadcast Engine for message caching and catch-up
//! - **Channel 1**: Read by an inbound bridge task for real-time forwarding to the Aggregator
//!
//! On outbound publish: messages are broadcast via both the Engine (channel 0) and the direct
//! sender (channel 1, encoded to bytes via `Encode::encode()`).
//!
//! On inbound receive: channel 1 messages arrive via a Tokio mpsc bridge (commonware Receiver
//! bridged to Tokio), are deduplicated by SHA-256 digest, filtered by ServiceRouter, and
//! forwarded to the Aggregator as `AggregatorCommand::Receive`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use commonware_broadcast::buffered::{Config as BroadcastConfig, Engine};
use commonware_broadcast::Broadcaster;
use commonware_codec::{
    Decode, Encode, EncodeSize, Error as CodecError, RangeCfg, Read as CodecRead, ReadRangeExt,
    Write as CodecWrite,
};
use commonware_cryptography::{ed25519, sha256, Digestible, Hasher, Sha256};
use commonware_p2p::{Recipients, Sender as P2pSender};
use commonware_runtime::{Buf, BufMut};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utils::context::AppContext;
use wavs_types::{P2pStatus, ServiceId, Submission};

use super::{error::AggregatorError, peer::Peer, AggregatorCommand};

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
        /// Max message size in bytes (default: 65536 = 64KB)
        #[serde(default)]
        max_message_size: Option<u32>,
        /// Broadcast Engine deque size per peer for catch-up (default: 128)
        #[serde(default)]
        deque_size: Option<usize>,
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
        /// Max message size in bytes (default: 65536 = 64KB)
        #[serde(default)]
        max_message_size: Option<u32>,
        /// Broadcast Engine deque size per peer for catch-up (default: 128)
        #[serde(default)]
        deque_size: Option<usize>,
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

    /// Returns the configured max_message_size, or the default (65536 = 64KB).
    pub fn max_message_size(&self) -> u32 {
        match self {
            P2pConfig::Local {
                max_message_size, ..
            } => max_message_size.unwrap_or(65536),
            P2pConfig::Remote {
                max_message_size, ..
            } => max_message_size.unwrap_or(65536),
            P2pConfig::Disabled => 65536,
        }
    }

    /// Returns the configured deque_size for the broadcast Engine, or the default (128).
    pub fn deque_size(&self) -> usize {
        match self {
            P2pConfig::Local { deque_size, .. } => deque_size.unwrap_or(128),
            P2pConfig::Remote { deque_size, .. } => deque_size.unwrap_or(128),
            P2pConfig::Disabled => 128,
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
        let payload = <Vec<u8>>::read_range(buf, *range)?;
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
    pub fn subscribed_services(&self) -> Vec<String> {
        self.subscribed_services
            .iter()
            .map(const_hex::encode)
            .collect()
    }

    /// Return raw bytes of subscribed service IDs (for building subscription announcements).
    pub fn subscribed_services_raw(&self) -> Vec<[u8; 32]> {
        self.subscribed_services.iter().copied().collect()
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
// Subscription Tracking (Per-Service P2P Targeting)
// ============================================================================

/// Subscription announcement carried as P2pMessage payload (ANN-05).
/// The service_id_bytes field of the wrapping P2pMessage is set to SUBSCRIPTION_SENTINEL.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SubscriptionAnnouncement {
    /// Services this peer is subscribing to
    pub subscribe: Vec<[u8; 32]>,
    /// Services this peer is unsubscribing from
    pub unsubscribe: Vec<[u8; 32]>,
    /// If true, `subscribe` is the FULL set of services (replace-not-merge).
    /// If false, `subscribe`/`unsubscribe` are incremental changes.
    /// Defaults to false for backward compatibility with Phase 14 announcements.
    #[serde(default)]
    pub full_state: bool,
}

impl SubscriptionAnnouncement {
    /// Encode as a P2pMessage with the subscription sentinel.
    pub fn to_p2p_message(&self) -> Result<P2pMessage, serde_json::Error> {
        let payload = serde_json::to_vec(self)?;
        Ok(P2pMessage {
            service_id_bytes: SUBSCRIPTION_SENTINEL,
            payload,
        })
    }

    /// Decode from a P2pMessage payload (caller must verify sentinel first).
    pub fn from_payload(payload: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(payload)
    }
}

/// Bidirectional subscription index tracking which peers subscribe to which services (SUB-01, SUB-02).
///
/// Maintains forward (service_id -> Set<PeerPubkey>) and reverse (PeerPubkey -> Set<service_id>)
/// indexes kept in sync. Single-threaded -- lives inside the bridge loop's tokio::select!.
pub(crate) struct PeerSubscriptionMap {
    /// Forward index: service_id -> set of subscribed peers
    service_to_peers: HashMap<[u8; 32], HashSet<ed25519::PublicKey>>,
    /// Reverse index: peer -> set of subscribed services (for disconnect cleanup)
    peer_to_services: HashMap<ed25519::PublicKey, HashSet<[u8; 32]>>,
}

impl PeerSubscriptionMap {
    pub fn new() -> Self {
        Self {
            service_to_peers: HashMap::new(),
            peer_to_services: HashMap::new(),
        }
    }

    /// Process a subscription announcement from a peer.
    /// Idempotent -- duplicate subscriptions are no-ops.
    pub fn handle_announcement(
        &mut self,
        peer: &ed25519::PublicKey,
        announcement: &SubscriptionAnnouncement,
    ) {
        for service_id in &announcement.subscribe {
            self.service_to_peers
                .entry(*service_id)
                .or_default()
                .insert(peer.clone());
            self.peer_to_services
                .entry(peer.clone())
                .or_default()
                .insert(*service_id);
        }
        for service_id in &announcement.unsubscribe {
            if let Some(peers) = self.service_to_peers.get_mut(service_id) {
                peers.remove(peer);
                if peers.is_empty() {
                    self.service_to_peers.remove(service_id);
                }
            }
            if let Some(services) = self.peer_to_services.get_mut(peer) {
                services.remove(service_id);
                if services.is_empty() {
                    self.peer_to_services.remove(peer);
                }
            }
        }
    }

    /// Remove all subscriptions for a disconnected peer (SUB-03).
    /// Uses the reverse index for efficient cleanup.
    pub fn remove_peer(&mut self, peer: &ed25519::PublicKey) {
        if let Some(services) = self.peer_to_services.remove(peer) {
            for service_id in services {
                if let Some(peers) = self.service_to_peers.get_mut(&service_id) {
                    peers.remove(peer);
                    if peers.is_empty() {
                        self.service_to_peers.remove(&service_id);
                    }
                }
            }
        }
    }

    /// Returns true if this peer has ever sent a subscription announcement (COMPAT-03).
    /// Peers that have never announced are treated as subscribed-to-all by callers.
    pub fn has_announced(&self, peer: &ed25519::PublicKey) -> bool {
        self.peer_to_services.contains_key(peer)
    }

    /// Returns the set of peers that have subscription entries in this map.
    /// Used by heartbeat pruning to compare against connected peers (SUB-03).
    pub fn tracked_peers(&self) -> HashSet<ed25519::PublicKey> {
        self.peer_to_services.keys().cloned().collect()
    }

    /// Replace all subscriptions for a peer with the given set.
    /// Uses replace-not-merge semantics for heartbeat/hello full state sync.
    pub fn set_peer_subscriptions(&mut self, peer: &ed25519::PublicKey, services: Vec<[u8; 32]>) {
        // Remove existing subscriptions first
        self.remove_peer(peer);
        // Then set the new full set (if non-empty)
        if !services.is_empty() {
            let service_set: HashSet<[u8; 32]> = services.iter().copied().collect();
            for service_id in &service_set {
                self.service_to_peers
                    .entry(*service_id)
                    .or_default()
                    .insert(peer.clone());
            }
            self.peer_to_services.insert(peer.clone(), service_set);
        }
    }

    /// Get the recipient set for targeted delivery.
    /// Includes un-announced connected peers for backward compatibility (COMPAT-03).
    /// Returns Recipients::Some(peers) if any peers are resolved, or Recipients::All as fallback.
    pub fn get_recipients(
        &self,
        service_id: &[u8; 32],
        connected_peers: &HashSet<ed25519::PublicKey>,
    ) -> Recipients<ed25519::PublicKey> {
        let mut result: HashSet<ed25519::PublicKey> = HashSet::new();

        // Add peers subscribed to this specific service
        if let Some(peers) = self.service_to_peers.get(service_id) {
            result.extend(peers.iter().cloned());
        }

        // COMPAT-03: Include connected peers that have never announced.
        // Pre-v1.3 nodes never send subscription announcements, so they must be
        // included unconditionally to maintain backward-compatible delivery.
        for peer in connected_peers {
            if !self.has_announced(peer) {
                result.insert(peer.clone());
            }
        }

        if result.is_empty() {
            Recipients::All
        } else {
            Recipients::Some(result.into_iter().collect())
        }
    }

    /// Returns per-service peer counts for observability (OBS-01).
    /// Keys are hex-encoded service_id bytes, values are the number of peers
    /// subscribed to that service.
    pub fn peer_subscription_counts(&self) -> HashMap<String, usize> {
        self.service_to_peers
            .iter()
            .map(|(service_id, peers)| (const_hex::encode(service_id), peers.len()))
            .collect()
    }
}

// ============================================================================
// Network Management (Commonware Runtime + Lookup Mode)
// ============================================================================

use commonware_cryptography::Signer;
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
    let socket_addr: std::net::SocketAddr = parts[1].parse().map_err(|e| {
        AggregatorError::P2p(format!("Invalid socket address in '{}': {}", addr, e))
    })?;
    Ok((pubkey, socket_addr))
}

/// Parse hex-encoded Ed25519 public key strings into PublicKey values.
fn parse_authorized_peers(hex_keys: &[String]) -> Result<Vec<ed25519::PublicKey>, AggregatorError> {
    hex_keys
        .iter()
        .map(|hex| {
            let bytes = const_hex::decode(hex).map_err(|e| {
                AggregatorError::P2p(format!("Invalid hex pubkey '{}': {}", hex, e))
            })?;
            pubkey_from_bytes(&bytes).map_err(|e| {
                AggregatorError::P2p(format!("Invalid Ed25519 pubkey '{}': {}", hex, e))
            })
        })
        .collect()
}

/// Parse a bootstrapper address string of format "<hex_pubkey>@<host>:<port>"
/// into a Bootstrapper tuple (PublicKey, Ingress).
///
/// Bootstrappers in discovery mode need their public key and a dialable address.
fn parse_bootstrapper(addr: &str) -> Result<(ed25519::PublicKey, Ingress), AggregatorError> {
    let (pubkey, socket_addr) = parse_peer_address(addr)?;
    Ok((pubkey, Ingress::from(socket_addr)))
}

/// Construct an Ed25519 PublicKey from raw bytes using commonware's codec.
fn pubkey_from_bytes(bytes: &[u8]) -> Result<ed25519::PublicKey, String> {
    use commonware_codec::ReadExt;
    let mut buf = bytes;
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
    aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
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
                        max_message_size,
                        deque_size,
                    } => {
                        let max_msg_size = max_message_size.unwrap_or(65536);
                        let deq_size = deque_size.unwrap_or(128);
                        run_lookup_network(
                            context,
                            &private_key,
                            listen_port,
                            peer_addresses,
                            authorized_peers,
                            command_rx,
                            aggregator_tx,
                            max_msg_size,
                            deq_size,
                        )
                        .await;
                    }
                    P2pConfig::Remote {
                        listen_port,
                        ref bootstrappers,
                        ref authorized_peers,
                        max_message_size,
                        deque_size,
                    } => {
                        let max_msg_size = max_message_size.unwrap_or(65536);
                        let deq_size = deque_size.unwrap_or(128);
                        run_discovery_network(
                            context,
                            &private_key,
                            listen_port,
                            bootstrappers,
                            authorized_peers,
                            command_rx,
                            aggregator_tx,
                            max_msg_size,
                            deq_size,
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

/// Reserved service ID used by heartbeat probes to discover connected peers.
/// No real service uses all-zeros service ID, so ServiceRouter filters these out.
const HEARTBEAT_SERVICE_ID: [u8; 32] = [0u8; 32];

/// Sentinel service_id for subscription announcement messages (ANN-05).
/// Distinguished from HEARTBEAT_SERVICE_ID ([0x00; 32]) and real service_id SHA-256 hashes.
/// A valid SHA-256 hash producing all-0xFF is astronomically unlikely (~1/2^256).
pub(crate) const SUBSCRIPTION_SENTINEL: [u8; 32] = [0xFF; 32];

/// Check if a P2pMessage is a subscription announcement by its sentinel service_id.
fn is_subscription_announcement(msg: &P2pMessage) -> bool {
    msg.service_id_bytes == SUBSCRIPTION_SENTINEL
}

/// Run a lookup-mode P2P network inside the commonware runtime.
///
/// This function:
/// 1. Creates a lookup::Network with the node's Ed25519 identity
/// 2. Configures the Oracle with authorized peers + own pubkey
/// 3. Registers two broadcast channels (Engine + direct forwarding)
/// 4. Creates the broadcast Engine for message caching and catch-up
/// 5. Spawns an inbound bridge task (commonware Receiver -> tokio mpsc)
/// 6. Starts the network and Engine
/// 7. Runs a bridge loop handling P2pCommands and inbound messages
///
/// SEC-02: Rate limiting is active via lookup::Config::local() which sets:
/// - allowed_connection_rate_per_peer: Quota::per_second(1)
/// - allowed_handshake_rate_per_ip: Quota::per_second(16)
/// - allowed_handshake_rate_per_subnet: Quota::per_second(128)
///
/// These are non-zero defaults confirmed from the Config::local() source.
#[allow(clippy::too_many_arguments)]
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
    aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    max_message_size: u32,
    deque_size_param: usize,
) {
    let listen_addr = std::net::SocketAddr::from(([0, 0, 0, 0], listen_port));

    // Create lookup network config with rate limiting active (Config::local defaults).
    // SEC-02 verified: Config::local() sets allowed_connection_rate_per_peer = per_second(1),
    // allowed_handshake_rate_per_ip = per_second(16), allowed_handshake_rate_per_subnet = per_second(128).
    let config = lookup::Config::local(
        private_key.clone(),
        b"wavs-p2p", // namespace for replay protection
        listen_addr,
        max_message_size, // configurable max_message_size (default: 64KB)
    );

    tracing::debug!(
        "P2P lookup config: rate limiting active (connection_rate_per_peer=1/s, handshake_rate_per_ip=16/s, handshake_rate_per_subnet=128/s)"
    );

    let (mut network, mut oracle) = lookup::Network::new(context.with_label("p2p_network"), config);

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
        match parse_authorized_peers(std::slice::from_ref(hex_key)) {
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

    // Register TWO channels before network.start():
    // Channel 0: for broadcast Engine (caching + catch-up via push-based re-broadcast)
    let (engine_sender, engine_receiver) = network.register(
        0u64,
        Quota::per_second(NZU32!(100)),
        1024, // backlog
    );

    // Channel 1: for direct message forwarding to Aggregator
    let (mut direct_sender, direct_receiver) = network.register(
        1u64,
        Quota::per_second(NZU32!(100)),
        1024, // backlog
    );

    // Create the broadcast Engine for message caching and catch-up.
    // CATCH-01: The Engine caches broadcast messages in per-peer deques (bounded by deque_size).
    // When a peer reconnects, the Engine's internal relay delivers cached messages from
    // connected peers' deques to the newly connected peer. This is push-based -- the Engine
    // automatically re-broadcasts cached content when peers reconnect. No application-level
    // pull mechanism (mailbox.subscribe(digest)) is needed for catch-up.
    // CATCH-02: deque_size bounds per-peer message storage. When full, oldest messages
    // are evicted. This prevents unbounded memory growth.
    let broadcast_config = BroadcastConfig {
        public_key: own_pubkey.clone(),
        mailbox_size: 256,
        deque_size: deque_size_param, // CATCH-02: bounded message storage per peer (configurable)
        priority: false,
        codec_config: (RangeCfg::new(0..=(max_message_size as usize)), ()), // P2pMessage::Cfg = (RangeCfg<usize>, ())
        peer_provider: oracle.clone(),
    };
    let (engine, mailbox) =
        Engine::<_, _, P2pMessage, _>::new(context.with_label("p2p_broadcast"), broadcast_config);

    // Start the network (consumes self, returns a handle)
    let _net_handle = network.start();

    // Start the broadcast Engine (consumes engine_sender/receiver for channel 0)
    let _engine_handle = engine.start((engine_sender, engine_receiver));

    // Bridge commonware Receiver (channel 1) -> Tokio mpsc channel.
    // The direct_receiver runs in the commonware runtime and may not be directly
    // compatible with tokio::select!. This dedicated bridge task reads from the
    // commonware Receiver and forwards to a Tokio mpsc channel.
    let (inbound_tx, mut inbound_rx) =
        tokio::sync::mpsc::channel::<(ed25519::PublicKey, commonware_runtime::IoBuf)>(256);
    {
        let inbound_tx = inbound_tx.clone();
        let mut direct_receiver = direct_receiver;
        context.clone().spawn(move |_ctx| async move {
            use commonware_p2p::Receiver as P2pReceiver;
            loop {
                match direct_receiver.recv().await {
                    Ok((peer_pubkey, raw_bytes)) => {
                        if inbound_tx.send((peer_pubkey, raw_bytes)).await.is_err() {
                            break; // Bridge loop shut down
                        }
                    }
                    Err(e) => {
                        tracing::error!("Direct receiver error: {:?}", e);
                        break;
                    }
                }
            }
        });
    }

    tracing::info!(
        "P2P network started (peer_id: {})",
        const_hex::encode(own_pubkey.as_ref())
    );

    // Bridge loop state
    let mut service_router = ServiceRouter::new();
    let mut retry_queue = RetryQueue::new();
    // BCAST-02: Application-level deduplication by message digest.
    // The Engine deduplicates on channel 0 internally, but channel 1
    // (direct) has no built-in dedup. This set ensures exactly-once
    // delivery to the Aggregator regardless of channel.
    let mut seen_digests: HashSet<sha256::Digest> = HashSet::new();
    const MAX_SEEN_DIGESTS: usize = 1024;

    // Phase 15: Peer subscription tracking
    let mut peer_subscriptions = PeerSubscriptionMap::new();
    // Track peers we have received any message from (for hello on first contact, ANN-04)
    let mut known_peers: HashSet<ed25519::PublicKey> = HashSet::new();
    // COMPAT-03: Connected peer set (PublicKey form) for backward-compat recipient resolution.
    // Updated from heartbeat and broadcast ack results.
    let mut connected_peer_set: HashSet<ed25519::PublicKey> = HashSet::new();

    // Connected peer tracking for OBS-01.
    // Updated from broadcast acknowledgment results and inbound message senders.
    // Starts at empty (truthful -- no peers confirmed until message exchange).
    let connected_peers_tracker: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));

    // Heartbeat timer: probes the P2P mesh every 2 seconds so connected_peers_tracker
    // is populated before any real trigger fires. Uses HEARTBEAT_SERVICE_ID (all-zeros
    // sentinel) which ServiceRouter always filters out -- never forwarded to aggregator.
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await; // consume the immediate first tick

    // Bridge loop: handle P2pCommands and inbound messages from peers.
    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                match cmd {
                    Some(P2pCommand::Publish { service_id, submission }) => {
                        match P2pMessage::from_submission(&service_id, &submission) {
                            Ok(msg) => {
                                // Broadcast via Engine channel (for caching + catch-up / CATCH-01)
                                let ack_rx = mailbox.broadcast(Recipients::All, msg.clone()).await;

                                // Channel 1: targeted delivery to subscribed peers only (TGT-01)
                                // get_recipients() returns Recipients::All as fallback when set is empty (TGT-02)
                                let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner(), &connected_peer_set);
                                let encoded_bytes = Encode::encode(&msg);
                                if let Err(e) = direct_sender.send(direct_recipients, encoded_bytes, false).await {
                                    tracing::warn!("Direct channel send failed: {:?}", e);
                                }

                                // Check Engine broadcast acknowledgment
                                match ack_rx.await {
                                    Ok(recipients) if recipients.is_empty() => {
                                        retry_queue.push(msg);
                                        tracing::warn!("No peers received broadcast, queued for retry");
                                    }
                                    Ok(recipients) => {
                                        // Update connected peer tracking from broadcast results (OBS-01)
                                        let peer_hexes: Vec<String> = recipients
                                            .iter()
                                            .map(|pk| const_hex::encode(pk.as_ref()))
                                            .collect();
                                        *connected_peers_tracker.write().unwrap() = peer_hexes;
                                        connected_peer_set = recipients.iter().cloned().collect();

                                        tracing::debug!("Broadcast delivered to {} peers", recipients.len());
                                        // Flush retry queue since peers are available
                                        if !retry_queue.is_empty() {
                                            let queued = retry_queue.drain_all();
                                            for queued_msg in queued {
                                                drop(mailbox.broadcast(Recipients::All, queued_msg.clone()).await);
                                                // TGT-04: Re-resolve recipients at drain time from current subscription state
                                                let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes, &connected_peer_set);
                                                let queued_bytes = Encode::encode(&queued_msg);
                                                if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
                                                    tracing::warn!("Direct channel retry send failed: {:?}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        tracing::error!("Broadcast engine shut down");
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to create P2pMessage: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::Subscribe { service_id }) => {
                        service_router.subscribe(&service_id);
                        tracing::info!("Subscribed to service: {}", service_id);
                        // ANN-01: Announce subscription to all connected peers
                        let announcement = SubscriptionAnnouncement {
                            subscribe: vec![service_id.inner()],
                            unsubscribe: vec![],
                            full_state: false,
                        };
                        if let Ok(msg) = announcement.to_p2p_message() {
                            let encoded = Encode::encode(&msg);
                            if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                                tracing::debug!("Subscription announcement send failed: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::Unsubscribe { service_id }) => {
                        service_router.unsubscribe(&service_id);
                        tracing::info!("Unsubscribed from service: {}", service_id);
                        // ANN-02: Announce unsubscription to all connected peers
                        let announcement = SubscriptionAnnouncement {
                            subscribe: vec![],
                            unsubscribe: vec![service_id.inner()],
                            full_state: false,
                        };
                        if let Ok(msg) = announcement.to_p2p_message() {
                            let encoded = Encode::encode(&msg);
                            if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                                tracing::debug!("Unsubscription announcement send failed: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::GetStatus { response_tx }) => {
                        let peers = connected_peers_tracker.read().unwrap().clone();
                        let status = P2pStatus {
                            enabled: true,
                            discovery_mode: "local".to_string(),
                            local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
                            listen_addresses: vec![listen_addr.to_string()],
                            connected_peers: peers.len(),
                            peer_ids: peers,
                            subscribed_services: service_router.subscribed_services(),
                            peer_subscriptions: peer_subscriptions.peer_subscription_counts(),
                        };
                        let _ = response_tx.send(status);
                    }
                    Some(P2pCommand::BlockPeer { pubkey_hex }) => {
                        match parse_authorized_peers(std::slice::from_ref(&pubkey_hex)) {
                            Ok(keys) if !keys.is_empty() => {
                                oracle.block(keys[0].clone()).await;
                                tracing::info!("Blocked peer: {}", pubkey_hex);
                            }
                            _ => {
                                tracing::error!("Failed to parse pubkey for blocking: {}", pubkey_hex);
                            }
                        }
                    }
                    None => {
                        tracing::info!("P2P command channel closed, shutting down");
                        context.stop(0, None).await.ok();
                        break;
                    }
                }
            }
            msg = inbound_rx.recv() => {
                match msg {
                    Some((peer_pubkey, raw_bytes)) => {
                        // Track inbound peer as connected (OBS-01)
                        {
                            let sender_hex = const_hex::encode(peer_pubkey.as_ref());
                            let mut peers = connected_peers_tracker.write().unwrap();
                            if !peers.contains(&sender_hex) {
                                peers.push(sender_hex);
                            }
                        }

                        // Decode P2pMessage from raw bytes
                        let p2p_msg: P2pMessage = match P2pMessage::decode_cfg(
                            raw_bytes, &(RangeCfg::new(0..=(max_message_size as usize)), ())
                        ) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!("Failed to decode P2P message: {:?}", e);
                                continue;
                            }
                        };

                        // BCAST-02: Deduplication by digest.
                        // Compute digest and skip if already seen.
                        let digest = p2p_msg.digest();
                        if seen_digests.contains(&digest) {
                            tracing::trace!("Duplicate message filtered by digest");
                            continue;
                        }
                        // Bound the set: if at capacity, clear to prevent unbounded growth.
                        if seen_digests.len() >= MAX_SEEN_DIGESTS {
                            seen_digests.clear();
                            tracing::debug!("Cleared seen_digests set (reached {} capacity)", MAX_SEEN_DIGESTS);
                        }
                        seen_digests.insert(digest);

                        // ANN-04: Send hello on first contact with a new peer
                        if !known_peers.contains(&peer_pubkey) {
                            known_peers.insert(peer_pubkey.clone());
                            let my_services = service_router.subscribed_services_raw();
                            if !my_services.is_empty() {
                                let hello = SubscriptionAnnouncement {
                                    subscribe: my_services,
                                    unsubscribe: vec![],
                                    full_state: true,
                                };
                                if let Ok(msg) = hello.to_p2p_message() {
                                    let encoded = Encode::encode(&msg);
                                    if let Err(e) = direct_sender.send(
                                        Recipients::One(peer_pubkey.clone()),
                                        encoded,
                                        false,
                                    ).await {
                                        tracing::debug!("Hello announcement to new peer failed: {:?}", e);
                                    }
                                }
                            }
                        }

                        // Phase 15: Intercept subscription announcements before service filtering
                        if is_subscription_announcement(&p2p_msg) {
                            match SubscriptionAnnouncement::from_payload(&p2p_msg.payload) {
                                Ok(announcement) => {
                                    if announcement.full_state {
                                        // Replace-not-merge for full state updates (heartbeat/hello)
                                        peer_subscriptions.set_peer_subscriptions(
                                            &peer_pubkey,
                                            announcement.subscribe.clone(),
                                        );
                                    } else {
                                        // Incremental for event-driven announcements
                                        peer_subscriptions.handle_announcement(&peer_pubkey, &announcement);
                                    }
                                    tracing::debug!(
                                        "Subscription update from {}: +{} -{}{}",
                                        const_hex::encode(peer_pubkey.as_ref()),
                                        announcement.subscribe.len(),
                                        announcement.unsubscribe.len(),
                                        if announcement.full_state { " (full)" } else { "" },
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!("Invalid subscription announcement: {:?}", e);
                                }
                            }
                            continue; // Do not forward subscription announcements to Aggregator
                        }

                        // Service filtering (BCAST-05)
                        if !service_router.should_accept(&p2p_msg) {
                            tracing::trace!("Filtered message for unsubscribed service");
                            continue;
                        }

                        // Deserialize and forward to Aggregator
                        match p2p_msg.to_submission() {
                            Ok((_service_id, submission)) => {
                                let peer_id = const_hex::encode(peer_pubkey.as_ref());
                                if let Err(e) = aggregator_tx.send(AggregatorCommand::Receive {
                                    submission,
                                    peer: Peer::Other(peer_id),
                                }) {
                                    tracing::error!("Failed to forward to aggregator: {:?}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to deserialize submission: {:?}", e);
                            }
                        }
                    }
                    None => {
                        tracing::error!("Inbound bridge channel closed");
                        break;
                    }
                }
            }
            _ = heartbeat.tick() => {
                let probe = P2pMessage {
                    service_id_bytes: HEARTBEAT_SERVICE_ID,
                    payload: vec![],
                };
                let ack_rx = mailbox.broadcast(Recipients::All, probe.clone()).await;
                let encoded = Encode::encode(&probe);
                if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                    tracing::trace!("Heartbeat direct send failed: {:?}", e);
                }
                match ack_rx.await {
                    Ok(recipients) if !recipients.is_empty() => {
                        let peer_hexes: Vec<String> = recipients
                            .iter()
                            .map(|pk| const_hex::encode(pk.as_ref()))
                            .collect();
                        *connected_peers_tracker.write().unwrap() = peer_hexes;
                        tracing::debug!("Heartbeat: {} peers connected", recipients.len());

                        // Update connected peer set for COMPAT-03 recipient resolution
                        connected_peer_set = recipients.iter().cloned().collect();

                        // SUB-03: Prune subscription entries for departed peers.
                        // Heartbeat ack recipients are the authoritative connected peer set.
                        let tracked = peer_subscriptions.tracked_peers();
                        for departed in tracked.difference(&connected_peer_set) {
                            peer_subscriptions.remove_peer(departed);
                            known_peers.remove(departed);
                            tracing::debug!(
                                "Pruned departed peer from subscriptions: {}",
                                const_hex::encode(departed.as_ref()),
                            );
                        }

                        if !retry_queue.is_empty() {
                            let queued = retry_queue.drain_all();
                            for queued_msg in queued {
                                drop(mailbox.broadcast(Recipients::All, queued_msg.clone()).await);
                                // TGT-04: Re-resolve recipients at drain time from current subscription state
                                let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes, &connected_peer_set);
                                let queued_bytes = Encode::encode(&queued_msg);
                                if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
                                    tracing::warn!("Retry send failed: {:?}", e);
                                }
                            }
                        }
                    }
                    Ok(_) => tracing::trace!("Heartbeat: no peers connected yet"),
                    Err(_) => tracing::error!("Heartbeat: broadcast engine shut down"),
                }
                // ANN-03: Piggyback full subscription state for self-healing consistency
                let my_services = service_router.subscribed_services_raw();
                if !my_services.is_empty() {
                    let announcement = SubscriptionAnnouncement {
                        subscribe: my_services,
                        unsubscribe: vec![],
                        full_state: true,
                    };
                    if let Ok(msg) = announcement.to_p2p_message() {
                        let encoded = Encode::encode(&msg);
                        if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                            tracing::trace!("Heartbeat subscription announcement failed: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

/// Run a discovery-mode P2P network inside the commonware runtime.
///
/// This function:
/// 1. Creates a discovery::Network with the node's Ed25519 identity
/// 2. Configures the Oracle with authorized peers as a Set<PublicKey>
/// 3. Registers two broadcast channels (Engine + direct forwarding)
/// 4. Creates the broadcast Engine for message caching and catch-up
/// 5. Spawns an inbound bridge task (commonware Receiver -> tokio mpsc)
/// 6. Starts the network and Engine
/// 7. Runs a bridge loop handling P2pCommands and inbound messages
///
/// Discovery mode uses bootstrapper nodes for peer discovery (production).
/// Addresses are discovered dynamically through bootstrappers (no upfront addresses needed).
///
/// NET-01: Discovery-based peer discovery with bootstrappers
/// NET-04: Automatic reconnection (built-in to discovery::Network via dial_frequency)
#[allow(clippy::too_many_arguments)]
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
    aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    max_message_size: u32,
    deque_size_param: usize,
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
        max_message_size, // configurable max_message_size (default: 64KB)
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

    // Register TWO channels before network.start():
    // Channel 0: for broadcast Engine (caching + catch-up via push-based re-broadcast)
    let (engine_sender, engine_receiver) = network.register(
        0u64,
        Quota::per_second(NZU32!(100)),
        1024, // backlog
    );

    // Channel 1: for direct message forwarding to Aggregator
    let (mut direct_sender, direct_receiver) = network.register(
        1u64,
        Quota::per_second(NZU32!(100)),
        1024, // backlog
    );

    // Create the broadcast Engine for message caching and catch-up.
    // CATCH-01: Engine caches messages per-peer. When a peer reconnects,
    // cached messages are delivered via the Engine's internal relay (push-based).
    // No application-level pull mechanism needed.
    // CATCH-02: deque_size bounds per-peer message storage.
    let broadcast_config = BroadcastConfig {
        public_key: own_pubkey.clone(),
        mailbox_size: 256,
        deque_size: deque_size_param, // CATCH-02: bounded message storage per peer (configurable)
        priority: false,
        codec_config: (RangeCfg::new(0..=(max_message_size as usize)), ()), // P2pMessage::Cfg = (RangeCfg<usize>, ())
        peer_provider: oracle.clone(),
    };
    let (engine, mailbox) = Engine::<_, _, P2pMessage, _>::new(
        context.with_label("p2p_discovery_broadcast"),
        broadcast_config,
    );

    // Start the network (consumes self, returns a handle)
    let _net_handle = network.start();

    // Start the broadcast Engine (consumes engine_sender/receiver for channel 0)
    let _engine_handle = engine.start((engine_sender, engine_receiver));

    // Bridge commonware Receiver (channel 1) -> Tokio mpsc channel.
    let (inbound_tx, mut inbound_rx) =
        tokio::sync::mpsc::channel::<(ed25519::PublicKey, commonware_runtime::IoBuf)>(256);
    {
        let inbound_tx = inbound_tx.clone();
        let mut direct_receiver = direct_receiver;
        context.clone().spawn(move |_ctx| async move {
            use commonware_p2p::Receiver as P2pReceiver;
            loop {
                match direct_receiver.recv().await {
                    Ok((peer_pubkey, raw_bytes)) => {
                        if inbound_tx.send((peer_pubkey, raw_bytes)).await.is_err() {
                            break; // Bridge loop shut down
                        }
                    }
                    Err(e) => {
                        tracing::error!("Discovery direct receiver error: {:?}", e);
                        break;
                    }
                }
            }
        });
    }

    tracing::info!(
        "P2P discovery network started (peer_id: {})",
        const_hex::encode(own_pubkey.as_ref())
    );

    // Bridge loop state
    let mut service_router = ServiceRouter::new();
    let mut retry_queue = RetryQueue::new();
    // BCAST-02: Application-level deduplication by message digest.
    let mut seen_digests: HashSet<sha256::Digest> = HashSet::new();
    const MAX_SEEN_DIGESTS: usize = 1024;

    // Phase 15: Peer subscription tracking
    let mut peer_subscriptions = PeerSubscriptionMap::new();
    // Track peers we have received any message from (for hello on first contact, ANN-04)
    let mut known_peers: HashSet<ed25519::PublicKey> = HashSet::new();
    // COMPAT-03: Connected peer set (PublicKey form) for backward-compat recipient resolution.
    // Updated from heartbeat and broadcast ack results.
    let mut connected_peer_set: HashSet<ed25519::PublicKey> = HashSet::new();

    // Connected peer tracking for OBS-01.
    // Updated from broadcast acknowledgment results and inbound message senders.
    // Starts at empty (truthful -- no peers confirmed until message exchange).
    let connected_peers_tracker: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));

    // Heartbeat timer: probes the P2P mesh every 2 seconds so connected_peers_tracker
    // is populated before any real trigger fires. Uses HEARTBEAT_SERVICE_ID (all-zeros
    // sentinel) which ServiceRouter always filters out -- never forwarded to aggregator.
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await; // consume the immediate first tick

    // Bridge loop: handle P2pCommands and inbound messages from peers.
    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                match cmd {
                    Some(P2pCommand::Publish { service_id, submission }) => {
                        match P2pMessage::from_submission(&service_id, &submission) {
                            Ok(msg) => {
                                let ack_rx = mailbox.broadcast(Recipients::All, msg.clone()).await;
                                // Channel 1: targeted delivery to subscribed peers only (TGT-01)
                                // get_recipients() returns Recipients::All as fallback when set is empty (TGT-02)
                                let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner(), &connected_peer_set);
                                let encoded_bytes = Encode::encode(&msg);
                                if let Err(e) = direct_sender.send(direct_recipients, encoded_bytes, false).await {
                                    tracing::warn!("Discovery direct channel send failed: {:?}", e);
                                }
                                match ack_rx.await {
                                    Ok(recipients) if recipients.is_empty() => {
                                        retry_queue.push(msg);
                                        tracing::warn!("No peers received broadcast, queued for retry");
                                    }
                                    Ok(recipients) => {
                                        // Update connected peer tracking from broadcast results (OBS-01)
                                        let peer_hexes: Vec<String> = recipients
                                            .iter()
                                            .map(|pk| const_hex::encode(pk.as_ref()))
                                            .collect();
                                        *connected_peers_tracker.write().unwrap() = peer_hexes;
                                        connected_peer_set = recipients.iter().cloned().collect();

                                        tracing::debug!("Broadcast delivered to {} peers", recipients.len());
                                        if !retry_queue.is_empty() {
                                            let queued = retry_queue.drain_all();
                                            for queued_msg in queued {
                                                drop(mailbox.broadcast(Recipients::All, queued_msg.clone()).await);
                                                // TGT-04: Re-resolve recipients at drain time from current subscription state
                                                let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes, &connected_peer_set);
                                                let queued_bytes = Encode::encode(&queued_msg);
                                                if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
                                                    tracing::warn!("Discovery direct channel retry send failed: {:?}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        tracing::error!("Broadcast engine shut down");
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to create P2pMessage: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::Subscribe { service_id }) => {
                        service_router.subscribe(&service_id);
                        tracing::info!("Subscribed to service: {}", service_id);
                        // ANN-01: Announce subscription to all connected peers
                        let announcement = SubscriptionAnnouncement {
                            subscribe: vec![service_id.inner()],
                            unsubscribe: vec![],
                            full_state: false,
                        };
                        if let Ok(msg) = announcement.to_p2p_message() {
                            let encoded = Encode::encode(&msg);
                            if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                                tracing::debug!("Subscription announcement send failed: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::Unsubscribe { service_id }) => {
                        service_router.unsubscribe(&service_id);
                        tracing::info!("Unsubscribed from service: {}", service_id);
                        // ANN-02: Announce unsubscription to all connected peers
                        let announcement = SubscriptionAnnouncement {
                            subscribe: vec![],
                            unsubscribe: vec![service_id.inner()],
                            full_state: false,
                        };
                        if let Ok(msg) = announcement.to_p2p_message() {
                            let encoded = Encode::encode(&msg);
                            if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                                tracing::debug!("Unsubscription announcement send failed: {:?}", e);
                            }
                        }
                    }
                    Some(P2pCommand::GetStatus { response_tx }) => {
                        let peers = connected_peers_tracker.read().unwrap().clone();
                        let status = P2pStatus {
                            enabled: true,
                            discovery_mode: "remote".to_string(),
                            local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
                            listen_addresses: vec![listen_addr.to_string()],
                            connected_peers: peers.len(),
                            peer_ids: peers,
                            subscribed_services: service_router.subscribed_services(),
                            peer_subscriptions: peer_subscriptions.peer_subscription_counts(),
                        };
                        let _ = response_tx.send(status);
                    }
                    Some(P2pCommand::BlockPeer { pubkey_hex }) => {
                        match parse_authorized_peers(std::slice::from_ref(&pubkey_hex)) {
                            Ok(keys) if !keys.is_empty() => {
                                oracle.block(keys[0].clone()).await;
                                tracing::info!("Blocked peer: {}", pubkey_hex);
                            }
                            _ => {
                                tracing::error!("Failed to parse pubkey for blocking: {}", pubkey_hex);
                            }
                        }
                    }
                    None => {
                        tracing::info!("P2P command channel closed, shutting down discovery network");
                        context.stop(0, None).await.ok();
                        break;
                    }
                }
            }
            msg = inbound_rx.recv() => {
                match msg {
                    Some((peer_pubkey, raw_bytes)) => {
                        // Track inbound peer as connected (OBS-01)
                        {
                            let sender_hex = const_hex::encode(peer_pubkey.as_ref());
                            let mut peers = connected_peers_tracker.write().unwrap();
                            if !peers.contains(&sender_hex) {
                                peers.push(sender_hex);
                            }
                        }

                        // Decode P2pMessage from raw bytes
                        let p2p_msg: P2pMessage = match P2pMessage::decode_cfg(
                            raw_bytes, &(RangeCfg::new(0..=(max_message_size as usize)), ())
                        ) {
                            Ok(m) => m,
                            Err(e) => {
                                tracing::warn!("Failed to decode P2P message: {:?}", e);
                                continue;
                            }
                        };

                        // BCAST-02: Deduplication by digest.
                        let digest = p2p_msg.digest();
                        if seen_digests.contains(&digest) {
                            tracing::trace!("Duplicate message filtered by digest");
                            continue;
                        }
                        if seen_digests.len() >= MAX_SEEN_DIGESTS {
                            seen_digests.clear();
                            tracing::debug!("Cleared seen_digests set (reached {} capacity)", MAX_SEEN_DIGESTS);
                        }
                        seen_digests.insert(digest);

                        // ANN-04: Send hello on first contact with a new peer
                        if !known_peers.contains(&peer_pubkey) {
                            known_peers.insert(peer_pubkey.clone());
                            let my_services = service_router.subscribed_services_raw();
                            if !my_services.is_empty() {
                                let hello = SubscriptionAnnouncement {
                                    subscribe: my_services,
                                    unsubscribe: vec![],
                                    full_state: true,
                                };
                                if let Ok(msg) = hello.to_p2p_message() {
                                    let encoded = Encode::encode(&msg);
                                    if let Err(e) = direct_sender.send(
                                        Recipients::One(peer_pubkey.clone()),
                                        encoded,
                                        false,
                                    ).await {
                                        tracing::debug!("Hello announcement to new peer failed: {:?}", e);
                                    }
                                }
                            }
                        }

                        // Phase 15: Intercept subscription announcements before service filtering
                        if is_subscription_announcement(&p2p_msg) {
                            match SubscriptionAnnouncement::from_payload(&p2p_msg.payload) {
                                Ok(announcement) => {
                                    if announcement.full_state {
                                        // Replace-not-merge for full state updates (heartbeat/hello)
                                        peer_subscriptions.set_peer_subscriptions(
                                            &peer_pubkey,
                                            announcement.subscribe.clone(),
                                        );
                                    } else {
                                        // Incremental for event-driven announcements
                                        peer_subscriptions.handle_announcement(&peer_pubkey, &announcement);
                                    }
                                    tracing::debug!(
                                        "Subscription update from {}: +{} -{}{}",
                                        const_hex::encode(peer_pubkey.as_ref()),
                                        announcement.subscribe.len(),
                                        announcement.unsubscribe.len(),
                                        if announcement.full_state { " (full)" } else { "" },
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!("Invalid subscription announcement: {:?}", e);
                                }
                            }
                            continue; // Do not forward subscription announcements to Aggregator
                        }

                        // Service filtering (BCAST-05)
                        if !service_router.should_accept(&p2p_msg) {
                            tracing::trace!("Filtered message for unsubscribed service");
                            continue;
                        }

                        // Deserialize and forward to Aggregator
                        match p2p_msg.to_submission() {
                            Ok((_service_id, submission)) => {
                                let peer_id = const_hex::encode(peer_pubkey.as_ref());
                                if let Err(e) = aggregator_tx.send(AggregatorCommand::Receive {
                                    submission,
                                    peer: Peer::Other(peer_id),
                                }) {
                                    tracing::error!("Failed to forward to aggregator: {:?}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to deserialize submission: {:?}", e);
                            }
                        }
                    }
                    None => {
                        tracing::error!("Discovery inbound bridge channel closed");
                        break;
                    }
                }
            }
            _ = heartbeat.tick() => {
                let probe = P2pMessage {
                    service_id_bytes: HEARTBEAT_SERVICE_ID,
                    payload: vec![],
                };
                let ack_rx = mailbox.broadcast(Recipients::All, probe.clone()).await;
                let encoded = Encode::encode(&probe);
                if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                    tracing::trace!("Heartbeat direct send failed: {:?}", e);
                }
                match ack_rx.await {
                    Ok(recipients) if !recipients.is_empty() => {
                        let peer_hexes: Vec<String> = recipients
                            .iter()
                            .map(|pk| const_hex::encode(pk.as_ref()))
                            .collect();
                        *connected_peers_tracker.write().unwrap() = peer_hexes;
                        tracing::debug!("Heartbeat: {} peers connected", recipients.len());

                        // Update connected peer set for COMPAT-03 recipient resolution
                        connected_peer_set = recipients.iter().cloned().collect();

                        // SUB-03: Prune subscription entries for departed peers.
                        // Heartbeat ack recipients are the authoritative connected peer set.
                        let tracked = peer_subscriptions.tracked_peers();
                        for departed in tracked.difference(&connected_peer_set) {
                            peer_subscriptions.remove_peer(departed);
                            known_peers.remove(departed);
                            tracing::debug!(
                                "Pruned departed peer from subscriptions: {}",
                                const_hex::encode(departed.as_ref()),
                            );
                        }

                        if !retry_queue.is_empty() {
                            let queued = retry_queue.drain_all();
                            for queued_msg in queued {
                                drop(mailbox.broadcast(Recipients::All, queued_msg.clone()).await);
                                // TGT-04: Re-resolve recipients at drain time from current subscription state
                                let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes, &connected_peer_set);
                                let queued_bytes = Encode::encode(&queued_msg);
                                if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
                                    tracing::warn!("Retry send failed: {:?}", e);
                                }
                            }
                        }
                    }
                    Ok(_) => tracing::trace!("Heartbeat: no peers connected yet"),
                    Err(_) => tracing::error!("Heartbeat: broadcast engine shut down"),
                }
                // ANN-03: Piggyback full subscription state for self-healing consistency
                let my_services = service_router.subscribed_services_raw();
                if !my_services.is_empty() {
                    let announcement = SubscriptionAnnouncement {
                        subscribe: my_services,
                        unsubscribe: vec![],
                        full_state: true,
                    };
                    if let Ok(msg) = announcement.to_p2p_message() {
                        let encoded = Encode::encode(&msg);
                        if let Err(e) = direct_sender.send(Recipients::All, encoded, false).await {
                            tracing::trace!("Heartbeat subscription announcement failed: {:?}", e);
                        }
                    }
                }
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
        aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
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
        let _thread_handle =
            spawn_commonware_runtime(private_key, p2p_config, command_rx, aggregator_tx)?;

        // TODO: Store thread_handle for clean shutdown in Phase 3

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
            .map_err(|e| AggregatorError::P2p(format!("Failed to send unsubscribe command: {}", e)))
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
pub fn ed25519_signer_from_mnemonic(
    mnemonic: &str,
) -> Result<ed25519::PrivateKey, AggregatorError> {
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
        let envelope_signature = WavsSignature::Secp256k1 {
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
        let decoded = P2pMessage::read_range(&mut encoded.as_ref(), 0..=65536).unwrap();

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
        assert_eq!(
            digest_a, digest_b,
            "Identical messages must produce same digest"
        );
        // Different messages produce different digests
        assert_ne!(
            digest_a, digest_c,
            "Different messages must produce different digests"
        );
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

        assert!(
            router.should_accept(&msg_a),
            "Should accept after subscribe"
        );
        router.unsubscribe(&service_id_a);
        assert!(
            !router.should_accept(&msg_a),
            "Should reject after unsubscribe"
        );
    }

    #[test]
    fn test_service_router_subscribed_services() {
        // subscribed_services() returns hex-encoded list of subscribed service IDs
        let service_id_a = ServiceId::hash(b"test-service-a");

        let mut router = ServiceRouter::new();
        assert!(router.subscribed_services().is_empty());

        router.subscribe(&service_id_a);
        let topics = router.subscribed_services();
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

    // ---- Subscription test helpers ----

    /// Create a deterministic ed25519 public key from a seed byte (for subscription tests).
    fn test_pubkey(seed_byte: u8) -> ed25519::PublicKey {
        use commonware_math::algebra::Random;
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha20Rng;
        let mut rng = ChaCha20Rng::from_seed([seed_byte; 32]);
        let private = ed25519::PrivateKey::random(&mut rng);
        private.public_key()
    }

    // ---- PeerSubscriptionMap tests ----

    #[test]
    fn test_peer_subscription_map_forward_index() {
        // SUB-01: handle_announcement with subscribe list populates forward index
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];

        let mut map = PeerSubscriptionMap::new();
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a, svc_b],
            unsubscribe: vec![],
            full_state: false,
        };
        map.handle_announcement(&peer_a, &announcement);

        // Forward index: both services should map to peer_a
        let empty_connected = HashSet::new();
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1);
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected Recipients::Some for svc_a, got {:?}", other),
        }
        match map.get_recipients(&svc_b, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1);
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected Recipients::Some for svc_b, got {:?}", other),
        }
    }

    #[test]
    fn test_peer_subscription_map_remove_peer() {
        // SUB-02: remove_peer clears peer from both forward and reverse indexes
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];

        let mut map = PeerSubscriptionMap::new();
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a, svc_b],
            unsubscribe: vec![],
            full_state: false,
        };
        map.handle_announcement(&peer_a, &announcement);

        // Verify peer is subscribed
        let empty_connected = HashSet::new();
        assert!(matches!(
            map.get_recipients(&svc_a, &empty_connected),
            Recipients::Some(_)
        ));

        // Remove peer
        map.remove_peer(&peer_a);

        // Both services should now fallback to Recipients::All
        assert!(matches!(
            map.get_recipients(&svc_a, &empty_connected),
            Recipients::All
        ));
        assert!(matches!(
            map.get_recipients(&svc_b, &empty_connected),
            Recipients::All
        ));
    }

    #[test]
    fn test_peer_subscription_map_disconnect_cleanup() {
        // SUB-03: remove_peer leaves no trace in either map
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];
        let svc_c = [0xCC; 32];

        let mut map = PeerSubscriptionMap::new();
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a, svc_b, svc_c],
            unsubscribe: vec![],
            full_state: false,
        };
        map.handle_announcement(&peer_a, &announcement);
        map.remove_peer(&peer_a);

        // Both internal maps should be completely empty
        assert!(
            map.service_to_peers.is_empty(),
            "Forward index should be empty after remove_peer"
        );
        assert!(
            map.peer_to_services.is_empty(),
            "Reverse index should be empty after remove_peer"
        );
    }

    #[test]
    fn test_subscription_announcement_roundtrip() {
        // ANN-05: SubscriptionAnnouncement round-trips through P2pMessage encoding
        let original = SubscriptionAnnouncement {
            subscribe: vec![[0xAA; 32], [0xBB; 32]],
            unsubscribe: vec![[0xCC; 32]],
            full_state: false,
        };

        let p2p_msg = original
            .to_p2p_message()
            .expect("to_p2p_message should succeed");
        assert_eq!(
            p2p_msg.service_id_bytes, SUBSCRIPTION_SENTINEL,
            "P2pMessage must use sentinel"
        );

        let recovered = SubscriptionAnnouncement::from_payload(&p2p_msg.payload)
            .expect("from_payload should succeed");
        assert_eq!(
            original, recovered,
            "Round-trip must produce identical announcement"
        );
    }

    #[test]
    fn test_subscription_sentinel_distinguishable() {
        // ANN-05: SUBSCRIPTION_SENTINEL is distinct from HEARTBEAT_SERVICE_ID and real service IDs
        assert_ne!(
            SUBSCRIPTION_SENTINEL, HEARTBEAT_SERVICE_ID,
            "Sentinel must differ from heartbeat"
        );

        // Real service IDs are SHA-256 hashes -- verify sentinel differs from a sample
        let real_service = ServiceId::hash(b"my-production-service");
        assert_ne!(
            SUBSCRIPTION_SENTINEL,
            real_service.inner(),
            "Sentinel must differ from real service IDs"
        );

        // is_subscription_announcement works correctly
        let sentinel_msg = P2pMessage {
            service_id_bytes: SUBSCRIPTION_SENTINEL,
            payload: vec![],
        };
        let heartbeat_msg = P2pMessage {
            service_id_bytes: HEARTBEAT_SERVICE_ID,
            payload: vec![],
        };
        let service_msg = P2pMessage {
            service_id_bytes: real_service.inner(),
            payload: vec![],
        };

        assert!(
            is_subscription_announcement(&sentinel_msg),
            "Sentinel message should be detected"
        );
        assert!(
            !is_subscription_announcement(&heartbeat_msg),
            "Heartbeat should not be detected as subscription"
        );
        assert!(
            !is_subscription_announcement(&service_msg),
            "Real service should not be detected as subscription"
        );
    }

    #[test]
    fn test_get_recipients_empty_fallback() {
        // SUB-01 fallback: get_recipients returns Recipients::All when no peers subscribed
        let map = PeerSubscriptionMap::new();
        let unknown_svc = [0xDD; 32];
        let empty_connected = HashSet::new();

        // Unknown service -> Recipients::All
        assert!(
            matches!(
                map.get_recipients(&unknown_svc, &empty_connected),
                Recipients::All
            ),
            "Unknown service must fallback to All"
        );

        // Subscribe then unsubscribe -> Recipients::All
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let mut map2 = PeerSubscriptionMap::new();
        map2.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map2.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![],
                unsubscribe: vec![svc_a],
                full_state: false,
            },
        );
        assert!(
            matches!(
                map2.get_recipients(&svc_a, &empty_connected),
                Recipients::All
            ),
            "Empty set after unsubscribe must fallback to All"
        );
    }

    #[test]
    fn test_peer_subscription_map_idempotent() {
        // Duplicate subscribe is a no-op (idempotent)
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        };
        map.handle_announcement(&peer_a, &announcement);
        map.handle_announcement(&peer_a, &announcement); // duplicate

        let empty_connected = HashSet::new();
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => assert_eq!(
                peers.len(),
                1,
                "Duplicate subscribe should not create duplicates"
            ),
            other => panic!("Expected Recipients::Some, got {:?}", other),
        }
    }

    #[test]
    fn test_peer_subscription_map_multiple_peers() {
        // Multiple peers subscribing to the same service
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map.handle_announcement(
            &peer_b,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        let empty_connected = HashSet::new();
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 2, "Both peers should be in recipient set");
                assert!(peers.contains(&peer_a));
                assert!(peers.contains(&peer_b));
            }
            other => panic!("Expected Recipients::Some with 2 peers, got {:?}", other),
        }

        // Remove one peer, other remains
        map.remove_peer(&peer_a);
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1, "Only peer_b should remain");
                assert!(peers.contains(&peer_b));
            }
            other => panic!("Expected Recipients::Some with 1 peer, got {:?}", other),
        }
    }

    #[test]
    fn test_peer_subscription_map_remove_unknown_peer() {
        // remove_peer on unknown peer is a no-op (no panic)
        let peer_a = test_pubkey(1);
        let mut map = PeerSubscriptionMap::new();
        map.remove_peer(&peer_a); // should not panic
        assert!(map.service_to_peers.is_empty());
        assert!(map.peer_to_services.is_empty());
    }

    // ---- ServiceRouter extension tests ----

    #[test]
    fn test_service_router_subscribed_services_raw() {
        // subscribed_services_raw returns raw [u8; 32] bytes
        let service_id_a = ServiceId::hash(b"test-service-a");
        let service_id_b = ServiceId::hash(b"test-service-b");

        let mut router = ServiceRouter::new();
        assert!(router.subscribed_services_raw().is_empty());

        router.subscribe(&service_id_a);
        router.subscribe(&service_id_b);

        let raw = router.subscribed_services_raw();
        assert_eq!(raw.len(), 2, "Should have 2 raw service IDs");
        assert!(
            raw.contains(&service_id_a.inner()),
            "Should contain service_a bytes"
        );
        assert!(
            raw.contains(&service_id_b.inner()),
            "Should contain service_b bytes"
        );
    }

    // ---- Phase 15 Plan 01: full_state, set_peer_subscriptions, has_announced ----

    #[test]
    fn test_full_state_serde_default() {
        // Deserialize JSON without full_state key -> defaults to false (backward compat)
        let json_no_field = r#"{"subscribe":[[170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170,170]],"unsubscribe":[]}"#;
        let deserialized: SubscriptionAnnouncement =
            serde_json::from_str(json_no_field).expect("Should deserialize without full_state");
        assert!(
            !deserialized.full_state,
            "Missing full_state should default to false"
        );
        assert_eq!(deserialized.subscribe.len(), 1);
        assert_eq!(deserialized.subscribe[0], [0xAA; 32]);

        // Explicit full_state=true round-trips
        let with_true = SubscriptionAnnouncement {
            subscribe: vec![[0xBB; 32]],
            unsubscribe: vec![],
            full_state: true,
        };
        let json = serde_json::to_string(&with_true).unwrap();
        let recovered: SubscriptionAnnouncement = serde_json::from_str(&json).unwrap();
        assert!(recovered.full_state, "Explicit true must round-trip");
        assert_eq!(recovered, with_true);
    }

    #[test]
    fn test_set_peer_subscriptions_replaces() {
        // set_peer_subscriptions replaces (not merges) existing subscriptions
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];
        let svc_c = [0xCC; 32];

        let mut map = PeerSubscriptionMap::new();
        // First subscribe to A and B via handle_announcement
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a, svc_b],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        let empty_connected = HashSet::new();
        assert!(matches!(
            map.get_recipients(&svc_a, &empty_connected),
            Recipients::Some(_)
        ));
        assert!(matches!(
            map.get_recipients(&svc_b, &empty_connected),
            Recipients::Some(_)
        ));

        // Now replace with only C
        map.set_peer_subscriptions(&peer_a, vec![svc_c]);

        // A and B should be gone (fallback to All)
        assert!(
            matches!(
                map.get_recipients(&svc_a, &empty_connected),
                Recipients::All
            ),
            "svc_a should fallback to All after replace"
        );
        assert!(
            matches!(
                map.get_recipients(&svc_b, &empty_connected),
                Recipients::All
            ),
            "svc_b should fallback to All after replace"
        );
        // C should be present
        match map.get_recipients(&svc_c, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1);
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected Recipients::Some for svc_c, got {:?}", other),
        }
    }

    #[test]
    fn test_set_peer_subscriptions_empty_removes() {
        // set_peer_subscriptions with empty vec removes peer entirely
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        let empty_connected = HashSet::new();
        map.set_peer_subscriptions(&peer_a, vec![svc_a]);
        assert!(matches!(
            map.get_recipients(&svc_a, &empty_connected),
            Recipients::Some(_)
        ));

        // Set to empty -> peer removed
        map.set_peer_subscriptions(&peer_a, vec![]);

        assert!(
            map.service_to_peers.is_empty(),
            "Forward index should be empty after set_peer_subscriptions([])"
        );
        assert!(
            map.peer_to_services.is_empty(),
            "Reverse index should be empty after set_peer_subscriptions([])"
        );
    }

    #[test]
    fn test_has_announced_compat03() {
        // COMPAT-03: has_announced tracks whether peer has sent any announcement
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        assert!(
            !map.has_announced(&peer_a),
            "Unknown peer should return false"
        );

        // After handle_announcement -> true
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        assert!(
            map.has_announced(&peer_a),
            "Peer should be announced after handle_announcement"
        );

        // After remove_peer -> false
        map.remove_peer(&peer_a);
        assert!(
            !map.has_announced(&peer_a),
            "Peer should not be announced after remove_peer"
        );
    }

    #[test]
    fn test_has_announced_after_set_peer_subscriptions() {
        // has_announced returns true after set_peer_subscriptions with non-empty services
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        assert!(!map.has_announced(&peer_a));

        map.set_peer_subscriptions(&peer_a, vec![svc_a]);
        assert!(
            map.has_announced(&peer_a),
            "Peer should be announced after set_peer_subscriptions"
        );
    }

    #[test]
    fn test_incremental_vs_full_state_processing() {
        // Validates data structure behavior for full_state vs incremental semantics.
        // The dispatch logic (if full_state -> set_peer_subscriptions else handle_announcement)
        // is a bridge loop concern (Plan 02), but we test the DATA STRUCTURE behavior here.
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];
        let svc_c = [0xCC; 32];

        let mut map = PeerSubscriptionMap::new();
        let empty_connected = HashSet::new();

        // Subscribe peer to svc_a via handle_announcement (incremental)
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        assert!(matches!(
            map.get_recipients(&svc_a, &empty_connected),
            Recipients::Some(_)
        ));

        // Simulate full_state=true: replace with svc_b only
        // Bridge loop would call set_peer_subscriptions for full_state=true
        map.set_peer_subscriptions(&peer_a, vec![svc_b]);

        // svc_a should be gone, svc_b should be present
        assert!(
            matches!(
                map.get_recipients(&svc_a, &empty_connected),
                Recipients::All
            ),
            "svc_a should be gone after full_state replace"
        );
        match map.get_recipients(&svc_b, &empty_connected) {
            Recipients::Some(peers) => assert!(peers.contains(&peer_a)),
            other => panic!("Expected Recipients::Some for svc_b, got {:?}", other),
        }

        // Simulate full_state=false: incremental add svc_c
        // Bridge loop would call handle_announcement for full_state=false
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_c],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        // Both svc_b and svc_c should be present (incremental merge)
        match map.get_recipients(&svc_b, &empty_connected) {
            Recipients::Some(peers) => assert!(peers.contains(&peer_a)),
            other => panic!(
                "Expected Recipients::Some for svc_b after incremental, got {:?}",
                other
            ),
        }
        match map.get_recipients(&svc_c, &empty_connected) {
            Recipients::Some(peers) => assert!(peers.contains(&peer_a)),
            other => panic!(
                "Expected Recipients::Some for svc_c after incremental, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_heartbeat_subscription_announcement() {
        // full_state=true announcement round-trips through P2pMessage encoding
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];

        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a, svc_b],
            unsubscribe: vec![],
            full_state: true,
        };

        let p2p_msg = announcement.to_p2p_message().expect("to_p2p_message");
        assert_eq!(
            p2p_msg.service_id_bytes, SUBSCRIPTION_SENTINEL,
            "Must use sentinel service_id"
        );

        let recovered =
            SubscriptionAnnouncement::from_payload(&p2p_msg.payload).expect("from_payload");
        assert!(recovered.full_state, "full_state must survive roundtrip");
        assert_eq!(recovered.subscribe.len(), 2);
        assert!(recovered.subscribe.contains(&svc_a));
        assert!(recovered.subscribe.contains(&svc_b));
        assert!(recovered.unsubscribe.is_empty());
    }

    #[test]
    fn test_subscribe_builds_announcement() {
        // Basic subscribe announcement: subscribe=[svc_a], unsubscribe=[], full_state=false
        let svc_a = [0xAA; 32];
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a],
            unsubscribe: vec![],
            full_state: false,
        };

        let p2p_msg = announcement.to_p2p_message().expect("to_p2p_message");
        assert_eq!(
            p2p_msg.service_id_bytes, SUBSCRIPTION_SENTINEL,
            "Subscribe announcement must use sentinel"
        );

        let recovered =
            SubscriptionAnnouncement::from_payload(&p2p_msg.payload).expect("from_payload");
        assert_eq!(recovered.subscribe, vec![svc_a]);
        assert!(recovered.unsubscribe.is_empty());
        assert!(!recovered.full_state);
    }

    #[test]
    fn test_unsubscribe_builds_announcement() {
        // ANN-02: Unsubscribe announcement round-trips correctly
        let svc_a = [0xAA; 32];
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![],
            unsubscribe: vec![svc_a],
            full_state: false,
        };

        let p2p_msg = announcement.to_p2p_message().expect("to_p2p_message");
        assert_eq!(
            p2p_msg.service_id_bytes, SUBSCRIPTION_SENTINEL,
            "Unsubscribe announcement must use SUBSCRIPTION_SENTINEL (ANN-02)"
        );

        let recovered =
            SubscriptionAnnouncement::from_payload(&p2p_msg.payload).expect("from_payload");
        assert!(recovered.subscribe.is_empty());
        assert_eq!(recovered.unsubscribe, vec![svc_a]);
        assert!(!recovered.full_state);
    }

    #[test]
    fn test_hello_on_first_contact() {
        // ANN-04: Hello-on-first-contact announcement preserves full_state=true and both services
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];
        let announcement = SubscriptionAnnouncement {
            subscribe: vec![svc_a, svc_b],
            unsubscribe: vec![],
            full_state: true,
        };

        let p2p_msg = announcement.to_p2p_message().expect("to_p2p_message");
        assert_eq!(
            p2p_msg.service_id_bytes, SUBSCRIPTION_SENTINEL,
            "Hello announcement must use SUBSCRIPTION_SENTINEL"
        );

        let recovered =
            SubscriptionAnnouncement::from_payload(&p2p_msg.payload).expect("from_payload");
        assert!(
            recovered.full_state,
            "Hello must preserve full_state=true (ANN-04)"
        );
        assert_eq!(recovered.subscribe.len(), 2);
        assert!(recovered.subscribe.contains(&svc_a));
        assert!(recovered.subscribe.contains(&svc_b));
        assert!(
            recovered.unsubscribe.is_empty(),
            "Hello should have empty unsubscribe"
        );
    }

    // TGT-04: Re-resolution returns different results before and after subscription state arrives
    #[test]
    fn test_retry_re_resolution() {
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        let empty_connected = HashSet::new();

        // Before any subscription state: get_recipients returns Recipients::All
        // This is what a message queued at startup would see if it cached recipients
        assert!(
            matches!(
                map.get_recipients(&svc_a, &empty_connected),
                Recipients::All
            ),
            "Before subscription state, must return Recipients::All"
        );

        // Simulate peer announcing subscription (as if subscription state arrived)
        map.set_peer_subscriptions(&peer_a, vec![svc_a]);

        // After subscription state: get_recipients returns Recipients::Some with peer_a
        // This proves re-resolution at drain time sees different results than cached All
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => {
                assert!(
                    peers.contains(&peer_a),
                    "After subscription, peers must contain peer_a"
                );
            }
            other => panic!(
                "Expected Recipients::Some after subscription state, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_peer_subscription_counts() {
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];

        let mut map = PeerSubscriptionMap::new();

        // Empty map returns empty counts
        assert!(map.peer_subscription_counts().is_empty());

        // After subscriptions, counts reflect state
        map.set_peer_subscriptions(&peer_a, vec![svc_a, svc_b]);
        map.set_peer_subscriptions(&peer_b, vec![svc_a]);

        let counts = map.peer_subscription_counts();
        assert_eq!(counts.len(), 2);
        assert_eq!(*counts.get(&const_hex::encode(svc_a)).unwrap(), 2);
        assert_eq!(*counts.get(&const_hex::encode(svc_b)).unwrap(), 1);

        // After removing a peer, counts update
        map.remove_peer(&peer_b);
        let counts = map.peer_subscription_counts();
        assert_eq!(*counts.get(&const_hex::encode(svc_a)).unwrap(), 1);
        assert_eq!(*counts.get(&const_hex::encode(svc_b)).unwrap(), 1);
    }

    // ---- Phase 18: SUB-03 tracked_peers and heartbeat prune tests ----

    #[test]
    fn test_tracked_peers_empty() {
        // tracked_peers() on a new map returns empty HashSet
        let map = PeerSubscriptionMap::new();
        assert!(
            map.tracked_peers().is_empty(),
            "New map should have no tracked peers"
        );
    }

    #[test]
    fn test_tracked_peers_returns_announced_peers() {
        // After handle_announcement for peer_a and peer_b, tracked_peers() returns both
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map.handle_announcement(
            &peer_b,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        let tracked = map.tracked_peers();
        assert_eq!(tracked.len(), 2, "Both peers should be tracked");
        assert!(tracked.contains(&peer_a));
        assert!(tracked.contains(&peer_b));
    }

    #[test]
    fn test_tracked_peers_after_remove() {
        // After handle_announcement then remove_peer, tracked_peers() no longer contains removed peer
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        assert!(map.tracked_peers().contains(&peer_a));

        map.remove_peer(&peer_a);
        assert!(
            !map.tracked_peers().contains(&peer_a),
            "Removed peer should not be tracked"
        );
        assert!(map.tracked_peers().is_empty());
    }

    #[test]
    fn test_heartbeat_prune_departed_peer() {
        // SUB-03: Simulates heartbeat-driven pruning of departed peers
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map.handle_announcement(
            &peer_b,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        // Both peers subscribed
        let empty_connected = HashSet::new();
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => assert_eq!(peers.len(), 2),
            other => panic!("Expected 2 peers, got {:?}", other),
        }

        // Heartbeat ack only returns peer_a -- peer_b departed
        let connected: HashSet<ed25519::PublicKey> = [peer_a.clone()].into_iter().collect();
        let tracked = map.tracked_peers();
        for departed in tracked.difference(&connected) {
            map.remove_peer(departed);
        }

        // After prune: only peer_a remains for svc_a
        match map.get_recipients(&svc_a, &connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1);
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected 1 peer, got {:?}", other),
        }
    }

    #[test]
    fn test_prune_noop_all_connected() {
        // Prune is a no-op when all tracked peers are still connected
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map.handle_announcement(
            &peer_b,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        // Both peers still connected
        let connected: HashSet<ed25519::PublicKey> =
            [peer_a.clone(), peer_b.clone()].into_iter().collect();
        let tracked = map.tracked_peers();
        for departed in tracked.difference(&connected) {
            map.remove_peer(departed);
        }

        // tracked_peers unchanged
        let tracked_after = map.tracked_peers();
        assert_eq!(tracked_after.len(), 2);
        assert!(tracked_after.contains(&peer_a));
        assert!(tracked_after.contains(&peer_b));
    }

    // ---- Phase 18: COMPAT-03 get_recipients with connected peers tests ----

    #[test]
    fn test_get_recipients_includes_unannounced_connected_peers() {
        // COMPAT-03: Un-announced connected peers are included in recipient set
        let peer_v13 = test_pubkey(1); // v1.3 peer (has announced)
        let peer_legacy = test_pubkey(2); // pre-v1.3 peer (never announced)
        let svc_a = [0xAA; 32];

        let mut map = PeerSubscriptionMap::new();

        // peer_v13 subscribes to svc_a
        map.handle_announcement(
            &peer_v13,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        // Both peers are connected
        let connected: HashSet<ed25519::PublicKey> = [peer_v13.clone(), peer_legacy.clone()]
            .into_iter()
            .collect();

        match map.get_recipients(&svc_a, &connected) {
            Recipients::Some(peers) => {
                assert!(peers.contains(&peer_v13), "v1.3 peer must be included");
                assert!(
                    peers.contains(&peer_legacy),
                    "Legacy peer must be included (COMPAT-03)"
                );
                assert_eq!(peers.len(), 2);
            }
            other => panic!("Expected Recipients::Some with 2 peers, got {:?}", other),
        }
    }

    #[test]
    fn test_get_recipients_all_announced_no_legacy() {
        // When all connected peers have announced, only subscribed peers are included
        let peer_a = test_pubkey(1);
        let peer_b = test_pubkey(2);
        let svc_a = [0xAA; 32];
        let svc_b = [0xBB; 32];

        let mut map = PeerSubscriptionMap::new();

        // peer_a subscribes to svc_a, peer_b subscribes to svc_b
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        map.handle_announcement(
            &peer_b,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_b],
                unsubscribe: vec![],
                full_state: false,
            },
        );

        // Both peers are connected and have announced
        let connected: HashSet<ed25519::PublicKey> =
            [peer_a.clone(), peer_b.clone()].into_iter().collect();

        match map.get_recipients(&svc_a, &connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1, "Only peer_a subscribes to svc_a");
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected Recipients::Some with 1 peer, got {:?}", other),
        }
    }

    #[test]
    fn test_get_recipients_empty_connected_set_preserves_old_behavior() {
        // get_recipients with empty connected set behaves exactly like old signature
        let peer_a = test_pubkey(1);
        let svc_a = [0xAA; 32];
        let unknown_svc = [0xDD; 32];

        let mut map = PeerSubscriptionMap::new();
        let empty_connected = HashSet::new();

        // No subscriptions -> Recipients::All
        assert!(matches!(
            map.get_recipients(&unknown_svc, &empty_connected),
            Recipients::All
        ));

        // With subscription -> Recipients::Some with subscribed peer only
        map.handle_announcement(
            &peer_a,
            &SubscriptionAnnouncement {
                subscribe: vec![svc_a],
                unsubscribe: vec![],
                full_state: false,
            },
        );
        match map.get_recipients(&svc_a, &empty_connected) {
            Recipients::Some(peers) => {
                assert_eq!(peers.len(), 1);
                assert!(peers.contains(&peer_a));
            }
            other => panic!("Expected Recipients::Some, got {:?}", other),
        }

        // Unknown service still falls back to All
        assert!(matches!(
            map.get_recipients(&unknown_svc, &empty_connected),
            Recipients::All
        ));
    }
}
