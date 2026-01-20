//! P2P Network Layer for WAVS Aggregator
//!
//! This module provides peer-to-peer networking for multi-operator WAVS deployments,
//! enabling operators to share submissions and reach quorum consensus.
//!
//! # Discovery Modes
//!
//! - **Local (mDNS)**: Uses multicast DNS for automatic peer discovery on local networks.
//!   Best for development and testing. Peers discover each other automatically.
//!
//! - **Remote (Kademlia)**: Uses a DHT for peer discovery across networks. Requires
//!   bootstrap nodes. One node runs as the bootstrap server (empty bootstrap_nodes),
//!   others connect to it. Periodic DHT queries ensure all peers eventually discover
//!   each other even if they join at different times.
//!
//! # Key Components
//!
//! - **GossipSub**: Pub/sub message dissemination per service topic
//! - **Request/Response**: Catch-up protocol for missed messages when peers reconnect
//! - **Identify**: Peer identification and address exchange
//! - **AutoNAT**: External address discovery for NAT traversal

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    io, iter,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures::{AsyncWriteExt, StreamExt};
use libp2p::{
    autonat,
    gossipsub::{self, IdentTopic, MessageAuthenticity, MessageId, ValidationMode},
    identify, kad, mdns,
    request_response::{self, Codec, ProtocolSupport},
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use utils::context::AppContext;
use wavs_types::{P2pStatus, ServiceId, Submission};

use super::{error::AggregatorError, peer::Peer, AggregatorCommand};

const PROTOCOL_VERSION: &str = "/wavs/1.0.0";
const CATCHUP_PROTOCOL: &str = "/wavs/catchup/1.0.0";

// ============================================================================
// Resource Limits & Timeouts
// ============================================================================
// These constants prevent unbounded resource consumption during network issues,
// peer misbehavior, or high load scenarios.

/// Maximum pending publishes in retry queue.
/// Prevents memory exhaustion during prolonged network partitions where publishes
/// accumulate faster than the 10-second retry timeout can clear them.
const MAX_PENDING_PUBLISHES: usize = 1000;

/// Maximum submissions stored per service for catch-up responses.
/// This is independent of `max_catchup_submissions` (which limits response size).
/// Prevents memory growth when receiving many submissions within the TTL window.
const MAX_STORED_SUBMISSIONS_PER_SERVICE: usize = 500;

/// Timeout for catch-up request/response protocol.
/// Explicitly set rather than relying on libp2p defaults for predictable behavior.
const CATCHUP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum concurrent outstanding catch-up requests per service.
/// Prevents overwhelming peers/network when many peers connect simultaneously.
/// Requests beyond this limit are skipped (peers cleared on response/failure allow retry).
const MAX_CONCURRENT_CATCHUP_REQUESTS_PER_SERVICE: usize = 3;

/// Pending publish entry for retry queue
struct PendingPublish {
    topic_name: String,
    data: Vec<u8>,
    created_at: Instant,
    retries: u32,
}

/// Stored submission for catch-up responses
struct StoredSubmission {
    submission: Submission,
    created_at: Instant,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum P2pConfig {
    /// Disabled - no P2P networking (for single-operator setups)
    #[default]
    Disabled,
    /// Local development - use mDNS for peer discovery with test-friendly defaults
    Local {
        /// Port to listen on for P2P connections (0 for random)
        listen_port: u16,
        max_retry_duration_secs: Option<u64>,
        retry_interval_ms: Option<u64>,
        submission_ttl_secs: Option<u64>,
        max_catchup_submissions: Option<usize>,
        cleanup_interval_secs: Option<u64>,
    },
    /// Remote/production - use Kademlia DHT for peer discovery with bootstrap nodes
    Remote {
        /// Port to listen on for P2P connections
        listen_port: u16,
        /// Bootstrap node addresses (multiaddr format). Empty = this node is a bootstrap server.
        bootstrap_nodes: Vec<String>,
        max_retry_duration_secs: Option<u64>,
        retry_interval_ms: Option<u64>,
        submission_ttl_secs: Option<u64>,
        max_catchup_submissions: Option<usize>,
        cleanup_interval_secs: Option<u64>,
        kademlia_discovery_interval_secs: Option<u64>,
    },
}

impl P2pConfig {
    const DEFAULT_MAX_RETRY_DURATION_SECS: u64 = 10;
    const DEFAULT_RETRY_INTERVAL_MS: u64 = 200;
    /// How long to keep submissions in memory for catch-up responses
    const DEFAULT_SUBMISSION_TTL_SECS: u64 = 300; // 5 minutes
    /// Maximum submissions to return in a catch-up response
    const DEFAULT_MAX_CATCHUP_SUBMISSIONS: usize = 100;
    /// This ensures peers eventually discover each other even if they join at different times.
    const DEFAULT_KADEMLIA_DISCOVERY_INTERVAL_SECS: u64 = 60;
    /// Interval between cleanup of expired stored submissions
    const DEFAULT_CLEANUP_INTERVAL_SECS: u64 = 60;

    pub fn cleanup_interval_secs(&self) -> u64 {
        match self {
            P2pConfig::Local {
                cleanup_interval_secs,
                ..
            } => cleanup_interval_secs.unwrap_or(Self::DEFAULT_CLEANUP_INTERVAL_SECS),
            P2pConfig::Remote {
                cleanup_interval_secs,
                ..
            } => cleanup_interval_secs.unwrap_or(Self::DEFAULT_CLEANUP_INTERVAL_SECS),
            P2pConfig::Disabled => Self::DEFAULT_CLEANUP_INTERVAL_SECS,
        }
    }

    pub fn kademlia_discovery_interval_secs(&self) -> u64 {
        match self {
            P2pConfig::Remote {
                kademlia_discovery_interval_secs,
                ..
            } => kademlia_discovery_interval_secs
                .unwrap_or(Self::DEFAULT_KADEMLIA_DISCOVERY_INTERVAL_SECS),
            P2pConfig::Local { .. } | P2pConfig::Disabled => {
                Self::DEFAULT_KADEMLIA_DISCOVERY_INTERVAL_SECS
            }
        }
    }

    pub fn max_retry_duration_secs(&self) -> u64 {
        match self {
            P2pConfig::Local {
                max_retry_duration_secs,
                ..
            } => max_retry_duration_secs.unwrap_or(Self::DEFAULT_MAX_RETRY_DURATION_SECS),
            P2pConfig::Remote {
                max_retry_duration_secs,
                ..
            } => max_retry_duration_secs.unwrap_or(Self::DEFAULT_MAX_RETRY_DURATION_SECS),
            P2pConfig::Disabled => Self::DEFAULT_MAX_RETRY_DURATION_SECS,
        }
    }

    pub fn retry_interval_ms(&self) -> u64 {
        match self {
            P2pConfig::Local {
                retry_interval_ms, ..
            } => retry_interval_ms.unwrap_or(Self::DEFAULT_RETRY_INTERVAL_MS),
            P2pConfig::Remote {
                retry_interval_ms, ..
            } => retry_interval_ms.unwrap_or(Self::DEFAULT_RETRY_INTERVAL_MS),
            P2pConfig::Disabled => Self::DEFAULT_RETRY_INTERVAL_MS,
        }
    }

    pub fn submission_ttl_secs(&self) -> u64 {
        match self {
            P2pConfig::Local {
                submission_ttl_secs,
                ..
            } => submission_ttl_secs.unwrap_or(Self::DEFAULT_SUBMISSION_TTL_SECS),
            P2pConfig::Remote {
                submission_ttl_secs,
                ..
            } => submission_ttl_secs.unwrap_or(Self::DEFAULT_SUBMISSION_TTL_SECS),
            P2pConfig::Disabled => Self::DEFAULT_SUBMISSION_TTL_SECS,
        }
    }

    pub fn max_catchup_submissions(&self) -> usize {
        match self {
            P2pConfig::Local {
                max_catchup_submissions,
                ..
            } => max_catchup_submissions.unwrap_or(Self::DEFAULT_MAX_CATCHUP_SUBMISSIONS),
            P2pConfig::Remote {
                max_catchup_submissions,
                ..
            } => max_catchup_submissions.unwrap_or(Self::DEFAULT_MAX_CATCHUP_SUBMISSIONS),
            P2pConfig::Disabled => Self::DEFAULT_MAX_CATCHUP_SUBMISSIONS,
        }
    }

    /// GossipSub heartbeat interval - how often peers exchange mesh state
    pub fn heartbeat_interval(&self) -> Duration {
        match self {
            // Faster heartbeat for local testing to speed up mesh formation
            P2pConfig::Local { .. } => Duration::from_millis(100),
            // Production defaults
            P2pConfig::Remote { .. } | P2pConfig::Disabled => Duration::from_secs(1),
        }
    }

    /// Minimum number of peers in mesh before gossipsub considers it "low"
    /// When below this threshold, gossipsub will try to add more peers
    pub fn mesh_n_low(&self) -> usize {
        match self {
            // For local testing with 3 nodes, set to 0 to avoid "Mesh low" warnings
            // during dynamic topic subscription. Messages still propagate via flooding.
            P2pConfig::Local { .. } => 0,
            // Production default (D_lo)
            P2pConfig::Remote { .. } => 4,
            P2pConfig::Disabled => 2,
        }
    }

    /// Target number of peers in mesh
    pub fn mesh_n(&self) -> usize {
        match self {
            // For local testing with 3 nodes, max possible mesh is 2 peers
            // Setting target to 1 is achievable even with just 2 connected nodes
            P2pConfig::Local { .. } => 1,
            // Production default (D)
            P2pConfig::Remote { .. } => 6,
            P2pConfig::Disabled => 6,
        }
    }

    /// Maximum number of peers in mesh
    pub fn mesh_n_high(&self) -> usize {
        match self {
            // For 3-node local testing, cap at 2 (max possible)
            P2pConfig::Local { .. } => 2,
            // Production default (D_hi)
            P2pConfig::Remote { .. } => 12,
            P2pConfig::Disabled => 12,
        }
    }

    /// How long to keep messages in cache for deduplication
    pub fn duplicate_cache_time(&self) -> Duration {
        match self {
            P2pConfig::Local { .. } => Duration::from_secs(30),
            P2pConfig::Remote { .. } | P2pConfig::Disabled => Duration::from_secs(60),
        }
    }

    /// History length for gossip (number of heartbeats)
    pub fn history_length(&self) -> usize {
        match self {
            P2pConfig::Local { .. } => 3,
            P2pConfig::Remote { .. } | P2pConfig::Disabled => 5,
        }
    }

    /// History gossip length (how many heartbeats of history to include)
    pub fn history_gossip(&self) -> usize {
        match self {
            P2pConfig::Local { .. } => 2,
            P2pConfig::Remote { .. } | P2pConfig::Disabled => 3,
        }
    }
}

// ============================================================================
// Catch-up Protocol Types
// ============================================================================

/// Request to catch up on missed submissions for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchUpRequest {
    /// Service ID to catch up on
    pub service_id: ServiceId,
}

/// Response containing missed submissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchUpResponse {
    /// Submissions the requester may have missed
    pub submissions: Vec<Submission>,
}

/// Codec for catch-up request/response protocol
#[derive(Debug, Clone, Default)]
pub struct CatchUpCodec;

#[async_trait]
impl Codec for CatchUpCodec {
    type Protocol = StreamProtocol;
    type Request = CatchUpRequest;
    type Response = CatchUpResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        use futures::AsyncReadExt;
        // Limit request size to prevent DoS - requests are small (just a ServiceId)
        const MAX_REQUEST_SIZE: u64 = 1024; // 1KB

        let mut buf = Vec::new();
        io.take(MAX_REQUEST_SIZE).read_to_end(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        use futures::AsyncReadExt;
        // Limit response size to prevent DoS - responses contain multiple submissions
        const MAX_RESPONSE_SIZE: u64 = 10 * 1024 * 1024; // 10MB

        let mut buf = Vec::new();
        io.take(MAX_RESPONSE_SIZE).read_to_end(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data =
            serde_json::to_vec(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&data).await?;
        io.close().await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data =
            serde_json::to_vec(&res).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&data).await?;
        io.close().await
    }
}

// ============================================================================
// Network Behaviour
// ============================================================================

/// libp2p behaviour for WAVS
#[derive(NetworkBehaviour)]
struct WavsBehaviour {
    /// GossipSub for message dissemination
    gossipsub: gossipsub::Behaviour,
    /// Request/Response for catch-up protocol
    catchup: request_response::Behaviour<CatchUpCodec>,
    /// mDNS for local peer discovery (Local mode only)
    mdns: Toggle<mdns::tokio::Behaviour>,
    /// Kademlia DHT for remote peer discovery (Remote mode only)
    kademlia: Toggle<kad::Behaviour<kad::store::MemoryStore>>,
    /// Identify protocol for peer identification
    identify: identify::Behaviour,
    /// AutoNAT for external address discovery
    autonat: autonat::Behaviour,
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
    pub async fn new(
        ctx: AppContext,
        p2p_config: P2pConfig,
        aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    ) -> Result<Option<Self>, AggregatorError> {
        if matches!(p2p_config, P2pConfig::Disabled) {
            tracing::info!("P2P networking is disabled");
            Ok(None)
        } else {
            let handle = Self::create_network(ctx, p2p_config, aggregator_tx).await?;
            Ok(Some(handle))
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

    /// Create a P2P network
    async fn create_network(
        ctx: AppContext,
        p2p_config: P2pConfig,
        aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    ) -> Result<Self, AggregatorError> {
        let swarm = build_swarm(&p2p_config)?;
        let local_peer_id = *swarm.local_peer_id();

        let mode_name = match p2p_config {
            P2pConfig::Local { .. } => "Local (mDNS)",
            P2pConfig::Remote { .. } => "Remote (Kademlia)",
            P2pConfig::Disabled => "Disabled",
        };
        tracing::info!("P2P peer ID: {} (mode: {})", local_peer_id, mode_name);

        let (command_tx, command_rx) = mpsc::unbounded_channel();

        tokio::spawn(run_event_loop(
            ctx,
            p2p_config,
            swarm,
            command_rx,
            aggregator_tx,
        ));

        Ok(P2pHandle { command_tx })
    }
}

// ============================================================================
// Swarm Building
// ============================================================================

/// Build the libp2p swarm with all required behaviours
fn build_swarm(config: &P2pConfig) -> Result<Swarm<WavsBehaviour>, AggregatorError> {
    // Message ID function for deduplication
    // Exclude sequence_number so the same content gets the same ID for proper deduplication
    let message_id_fn = |message: &gossipsub::Message| {
        let mut hasher = DefaultHasher::new();
        message.data.hash(&mut hasher);
        message.source.hash(&mut hasher);
        message.topic.hash(&mut hasher);
        MessageId::from(hasher.finish().to_string())
    };

    // GossipSub configuration - use config-specific values for test vs production
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(config.heartbeat_interval())
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .mesh_n_low(config.mesh_n_low())
        .mesh_n(config.mesh_n())
        .mesh_n_high(config.mesh_n_high())
        .duplicate_cache_time(config.duplicate_cache_time())
        .history_length(config.history_length())
        .history_gossip(config.history_gossip())
        .build()
        .map_err(|e| AggregatorError::P2p(format!("Failed to build gossipsub config: {}", e)))?;

    let is_local = matches!(config, P2pConfig::Local { .. });

    let swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|e| AggregatorError::P2p(format!("Failed to configure TCP: {}", e)))?
        .with_behaviour(|key| {
            let peer_id = key.public().to_peer_id();

            let gossipsub = gossipsub::Behaviour::new(
                MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .map_err(|e| format!("Failed to create gossipsub: {}", e))?;

            // Catch-up request/response protocol with explicit timeout
            let catchup = request_response::Behaviour::new(
                iter::once((StreamProtocol::new(CATCHUP_PROTOCOL), ProtocolSupport::Full)),
                request_response::Config::default().with_request_timeout(CATCHUP_REQUEST_TIMEOUT),
            );

            // Discovery: mDNS for Local mode, Kademlia for Remote mode
            let (mdns, kademlia) = if is_local {
                let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)
                    .map_err(|e| format!("Failed to create mDNS: {}", e))?;
                (Toggle::from(Some(mdns)), Toggle::from(None))
            } else {
                // Remote mode: use Kademlia DHT
                let store = kad::store::MemoryStore::new(peer_id);
                let kademlia = kad::Behaviour::new(peer_id, store);
                (Toggle::from(None), Toggle::from(Some(kademlia)))
            };

            let identify = identify::Behaviour::new(identify::Config::new(
                PROTOCOL_VERSION.to_string(),
                key.public(),
            ));

            let autonat = autonat::Behaviour::new(peer_id, autonat::Config::default());

            Ok(WavsBehaviour {
                gossipsub,
                catchup,
                mdns,
                kademlia,
                identify,
                autonat,
            })
        })
        .map_err(|e| AggregatorError::P2p(format!("Failed to build behaviour: {}", e)))?
        .build();

    Ok(swarm)
}

// ============================================================================
// Event Loop
// ============================================================================

/// State for the P2P event loop
struct EventLoopState {
    /// Topics we're subscribed to
    subscribed_topics: HashSet<String>,
    /// Service IDs we're subscribed to (for catch-up requests)
    subscribed_services: HashSet<ServiceId>,
    /// Pending publishes waiting for peers
    pending_publishes: VecDeque<PendingPublish>,
    /// Recent submissions stored for catch-up responses (service_id -> submissions)
    stored_submissions: HashMap<ServiceId, Vec<StoredSubmission>>,
    /// Peers we've already requested catch-up from, tracked per service
    catchup_requested_peers: HashMap<ServiceId, HashSet<PeerId>>,
    config: P2pConfig,
    /// Actual listen addresses (from NewListenAddr events, filtered for usable addresses)
    listen_addresses: Vec<Multiaddr>,
}

impl EventLoopState {
    fn new(config: P2pConfig) -> Self {
        Self {
            subscribed_topics: HashSet::new(),
            subscribed_services: HashSet::new(),
            pending_publishes: VecDeque::new(),
            stored_submissions: HashMap::new(),
            catchup_requested_peers: HashMap::new(),
            listen_addresses: Vec::new(),
            config,
        }
    }

    /// Store a submission for catch-up responses.
    /// Deduplicates by event ID and signer address, and enforces per-service storage limits.
    fn store_submission(&mut self, submission: Submission) {
        let service_id = submission.service_id().clone();

        // Check for duplicate by signer address
        let signer = submission
            .envelope_signature
            .evm_signer_address(&submission.envelope);

        let subs = self.stored_submissions.entry(service_id).or_default();

        // Skip if we already have a submission from this signer for this event
        if let Ok(signer_addr) = &signer {
            let already_exists = subs.iter().any(|s| {
                s.submission.event_id == submission.event_id
                    && s.submission
                        .envelope_signature
                        .evm_signer_address(&s.submission.envelope)
                        .map(|a| &a == signer_addr)
                        .unwrap_or(false)
            });
            if already_exists {
                tracing::debug!(
                    "Skipping duplicate submission from signer {} for event {}",
                    signer_addr,
                    submission.event_id
                );
                return;
            }
        }

        // Enforce per-service storage limit by removing oldest entries
        while subs.len() >= MAX_STORED_SUBMISSIONS_PER_SERVICE {
            subs.remove(0);
            tracing::debug!("Removed oldest stored submission due to storage limit");
        }

        let stored = StoredSubmission {
            submission,
            created_at: Instant::now(),
        };
        subs.push(stored);
    }

    /// Get submissions for a service (for catch-up response)
    fn get_submissions_for_catchup(&self, service_id: &ServiceId) -> Vec<Submission> {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.config.submission_ttl_secs());

        self.stored_submissions
            .get(service_id)
            .map(|subs| {
                subs.iter()
                    .filter(|s| now.duration_since(s.created_at) < ttl)
                    .take(self.config.max_catchup_submissions())
                    .map(|s| s.submission.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clean up expired submissions
    fn cleanup_expired_submissions(&mut self) {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.config.submission_ttl_secs());

        for subs in self.stored_submissions.values_mut() {
            subs.retain(|s| now.duration_since(s.created_at) < ttl);
        }

        // Remove empty service entries
        self.stored_submissions.retain(|_, subs| !subs.is_empty());
    }
}

/// Run the P2P event loop
async fn run_event_loop(
    ctx: AppContext,
    p2p_config: P2pConfig,
    mut swarm: Swarm<WavsBehaviour>,
    mut command_rx: mpsc::UnboundedReceiver<P2pCommand>,
    aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
) {
    let listen_port = match &p2p_config {
        P2pConfig::Local { listen_port, .. } => *listen_port,
        P2pConfig::Remote { listen_port, .. } => *listen_port,
        P2pConfig::Disabled => {
            tracing::error!("P2P is disabled, cannot run event loop");
            return;
        }
    };
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
        .parse()
        .expect("Valid multiaddr");

    if let Err(e) = swarm.listen_on(listen_addr.clone()) {
        tracing::error!("Failed to listen on {}: {}", listen_addr, e);
        return;
    }

    tracing::info!("P2P listening on {}", listen_addr);

    let bootstrap_nodes = match &p2p_config {
        P2pConfig::Remote {
            bootstrap_nodes, ..
        } => bootstrap_nodes,
        _ => &vec![],
    };

    // For Remote mode: dial bootstrap nodes and trigger Kademlia bootstrap
    if !bootstrap_nodes.is_empty() {
        for addr_str in bootstrap_nodes {
            match addr_str.parse::<Multiaddr>() {
                Ok(addr) => {
                    tracing::info!("Dialing bootstrap node: {}", addr);
                    if let Err(e) = swarm.dial(addr.clone()) {
                        tracing::warn!("Failed to dial bootstrap node {}: {:?}", addr, e);
                    }
                }
                Err(e) => {
                    tracing::error!("Invalid bootstrap address '{}': {}", addr_str, e);
                }
            }
        }

        // Trigger Kademlia bootstrap if available
        if let Some(kademlia) = swarm.behaviour_mut().kademlia.as_mut() {
            if let Err(e) = kademlia.bootstrap() {
                tracing::warn!("Kademlia bootstrap failed: {:?}", e);
            } else {
                tracing::info!("Kademlia bootstrap initiated");
            }
        }
    } else if swarm.behaviour().kademlia.as_ref().is_some() {
        tracing::info!("Running as bootstrap server (no bootstrap nodes configured)");
    }

    let mut retry_interval =
        tokio::time::interval(Duration::from_millis(p2p_config.retry_interval_ms()));
    let mut cleanup_interval =
        tokio::time::interval(Duration::from_secs(p2p_config.cleanup_interval_secs()));
    // Periodic peer discovery for Kademlia mode - helps find peers that joined after initial bootstrap
    let mut discovery_interval = tokio::time::interval(Duration::from_secs(
        p2p_config.kademlia_discovery_interval_secs(),
    ));
    let mut shutdown_signal = ctx.get_kill_receiver();
    let mut state = EventLoopState::new(p2p_config);

    loop {
        tokio::select! {
            _ = shutdown_signal.recv() => {
                tracing::info!("P2P network shutting down");
                break;
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, event, &aggregator_tx, &mut state);
            }
            Some(command) = command_rx.recv() => {
                handle_command(&mut swarm, command, &mut state);
            }
            _ = retry_interval.tick() => {
                retry_pending_publishes(&mut swarm, &mut state.pending_publishes, &state.config);
            }
            _ = cleanup_interval.tick() => {
                state.cleanup_expired_submissions();
            }
            _ = discovery_interval.tick() => {
                // Periodic peer discovery for Kademlia mode
                let local_peer_id = *swarm.local_peer_id();
                if let Some(kademlia) = swarm.behaviour_mut().kademlia.as_mut() {
                    kademlia.get_closest_peers(local_peer_id);
                }
            }
        }
    }
}

// ============================================================================
// Event Handlers
// ============================================================================

/// Retry any pending publishes in the queue.
/// Skips retries when no peers are connected to avoid wasting CPU cycles.
fn retry_pending_publishes(
    swarm: &mut Swarm<WavsBehaviour>,
    pending_publishes: &mut VecDeque<PendingPublish>,
    p2p_config: &P2pConfig,
) {
    // Skip retries entirely if no peers are connected - no point burning CPU
    if swarm.connected_peers().next().is_none() {
        return;
    }

    let now = Instant::now();
    let max_age = Duration::from_secs(p2p_config.max_retry_duration_secs());

    let mut items_to_retry: VecDeque<PendingPublish> = std::mem::take(pending_publishes);

    while let Some(mut item) = items_to_retry.pop_front() {
        if now.duration_since(item.created_at) > max_age {
            tracing::warn!(
                "P2P publish to {} timed out after {} retries",
                item.topic_name,
                item.retries
            );
            continue;
        }

        let topic = IdentTopic::new(&item.topic_name);
        match swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, item.data.clone())
        {
            Ok(_) => {
                tracing::debug!(
                    "Retry #{} successful: published to topic {}",
                    item.retries,
                    item.topic_name
                );
            }
            Err(gossipsub::PublishError::NoPeersSubscribedToTopic) => {
                item.retries += 1;
                pending_publishes.push_back(item);
            }
            Err(e) => {
                tracing::warn!(
                    "Retry #{} failed for topic {}: {}",
                    item.retries,
                    item.topic_name,
                    e
                );
                item.retries += 1;
                pending_publishes.push_back(item);
            }
        }
    }
}

/// Handle swarm events
fn handle_swarm_event(
    swarm: &mut Swarm<WavsBehaviour>,
    event: SwarmEvent<WavsBehaviourEvent>,
    aggregator_tx: &crossbeam::channel::Sender<AggregatorCommand>,
    state: &mut EventLoopState,
) {
    match event {
        // mDNS discovered new peers
        SwarmEvent::Behaviour(WavsBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
            for (peer_id, addr) in peers {
                tracing::info!("mDNS discovered peer: {} at {}", peer_id, addr);
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                if let Err(e) = swarm.dial(addr.clone()) {
                    tracing::debug!("Could not dial peer {} at {}: {:?}", peer_id, addr, e);
                }
            }
        }
        // mDNS peer expired
        SwarmEvent::Behaviour(WavsBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
            for (peer_id, _addr) in peers {
                tracing::info!("mDNS peer expired: {}", peer_id);
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
                // Allow re-requesting catch-up if peer reconnects - remove from all services
                for peer_set in state.catchup_requested_peers.values_mut() {
                    peer_set.remove(&peer_id);
                }
            }
        }
        // Received a gossipsub message
        SwarmEvent::Behaviour(WavsBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message,
            ..
        })) => {
            handle_gossip_message(propagation_source, message, aggregator_tx, state);
        }
        // Peer subscribed to a topic
        SwarmEvent::Behaviour(WavsBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
            peer_id,
            topic,
        })) => {
            tracing::debug!("Peer {} subscribed to topic {}", peer_id, topic);
        }
        // Peer unsubscribed from a topic
        SwarmEvent::Behaviour(WavsBehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed {
            peer_id,
            topic,
        })) => {
            tracing::debug!("Peer {} unsubscribed from topic {}", peer_id, topic);
        }
        // Catch-up request received
        SwarmEvent::Behaviour(WavsBehaviourEvent::Catchup(request_response::Event::Message {
            peer,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
            ..
        })) => {
            handle_catchup_request(swarm, peer, request, channel, state);
        }
        // Catch-up response received
        SwarmEvent::Behaviour(WavsBehaviourEvent::Catchup(request_response::Event::Message {
            peer,
            message: request_response::Message::Response { response, .. },
            ..
        })) => {
            handle_catchup_response(peer, response, aggregator_tx, state);
        }
        // Catch-up request/response errors
        SwarmEvent::Behaviour(WavsBehaviourEvent::Catchup(
            request_response::Event::OutboundFailure { peer, error, .. },
        )) => {
            tracing::warn!("Catch-up request to {} failed: {:?}", peer, error);
            // Clear peer from catchup_requested_peers to allow retry on next opportunity
            // (e.g., if the connection is still alive but the request just timed out)
            for peer_set in state.catchup_requested_peers.values_mut() {
                peer_set.remove(&peer);
            }
        }
        SwarmEvent::Behaviour(WavsBehaviourEvent::Catchup(
            request_response::Event::InboundFailure { peer, error, .. },
        )) => {
            tracing::warn!("Catch-up request from {} failed: {:?}", peer, error);
        }
        // Identify received - add peer addresses to Kademlia if available
        SwarmEvent::Behaviour(WavsBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            tracing::debug!(
                "Identified peer {}: {} with {} addresses",
                peer_id,
                info.protocol_version,
                info.listen_addrs.len()
            );
            // Add peer's addresses to Kademlia routing table if in Remote mode
            if let Some(kademlia) = swarm.behaviour_mut().kademlia.as_mut() {
                for addr in &info.listen_addrs {
                    if is_dialable_address(addr) {
                        kademlia.add_address(&peer_id, addr.clone());
                    }
                }
            }
        }
        // Kademlia routing table updated - peer discovered via DHT
        SwarmEvent::Behaviour(WavsBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
            peer,
            is_new_peer,
            addresses,
            ..
        })) => {
            if is_new_peer {
                tracing::debug!(
                    "Kademlia routing updated for peer: {} ({} addresses)",
                    peer,
                    addresses.len()
                );
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);

                // Dial the peer if not already connected
                if !swarm.is_connected(&peer) {
                    for addr in addresses.iter() {
                        if let Err(e) = swarm.dial(addr.clone()) {
                            tracing::debug!(
                                "Could not dial Kademlia peer {} at {}: {:?}",
                                peer,
                                addr,
                                e
                            );
                        } else {
                            tracing::debug!("Dialing Kademlia peer {} at {}", peer, addr);
                            break; // Only need to dial one address
                        }
                    }
                }
            }
        }
        // Kademlia query progress
        SwarmEvent::Behaviour(WavsBehaviourEvent::Kademlia(
            kad::Event::OutboundQueryProgressed { id, result, .. },
        )) => match result {
            kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { num_remaining, .. })) => {
                if num_remaining == 0 {
                    tracing::info!("Kademlia bootstrap complete");
                }
            }
            kad::QueryResult::Bootstrap(Err(e)) => {
                tracing::warn!("Kademlia bootstrap error: {:?}", e);
            }
            kad::QueryResult::GetClosestPeers(Ok(kad::GetClosestPeersOk { peers, .. })) => {
                tracing::debug!("Kademlia found {} closest peers", peers.len());
                for peer_info in peers {
                    // Dial the peer if not already connected (RoutingUpdated will handle gossipsub)
                    if !swarm.is_connected(&peer_info.peer_id) {
                        for addr in &peer_info.addrs {
                            if let Err(e) = swarm.dial(addr.clone()) {
                                tracing::debug!(
                                    "Could not dial closest peer {} at {}: {:?}",
                                    peer_info.peer_id,
                                    addr,
                                    e
                                );
                            } else {
                                tracing::debug!(
                                    "Dialing closest peer {} at {}",
                                    peer_info.peer_id,
                                    addr
                                );
                                break;
                            }
                        }
                    }
                }
            }
            _ => {
                tracing::debug!("Kademlia query {:?} progressed", id);
            }
        },
        // Other Kademlia events - debug level
        SwarmEvent::Behaviour(WavsBehaviourEvent::Kademlia(event)) => {
            tracing::debug!("Kademlia event: {:?}", event);
        }
        // AutoNAT status changed
        SwarmEvent::Behaviour(WavsBehaviourEvent::Autonat(autonat::Event::StatusChanged {
            old,
            new,
        })) => {
            tracing::info!("AutoNAT status changed: {:?} -> {:?}", old, new);
        }
        // AutoNAT inbound/outbound probes - debug level
        SwarmEvent::Behaviour(WavsBehaviourEvent::Autonat(event)) => {
            tracing::debug!("AutoNAT event: {:?}", event);
        }
        // New listen address - track usable addresses and add as external for Identify
        SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!("P2P listening on {}", address);
            if is_dialable_address(&address) {
                state.listen_addresses.push(address.clone());
                // Add as external address so Identify reports it to other peers
                swarm.add_external_address(address);
            }
        }
        // Connection established - request catch-up
        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            tracing::info!("Connection established with {} via {:?}", peer_id, endpoint);
            // Request catch-up for all subscribed services
            request_catchup_from_peer(swarm, peer_id, state);
        }
        // Connection closed
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            tracing::info!("Connection closed with {}: {:?}", peer_id, cause);
            // Remove from GossipSub explicit peers to prevent accumulation
            swarm
                .behaviour_mut()
                .gossipsub
                .remove_explicit_peer(&peer_id);
            // Allow re-requesting catch-up if peer reconnects - remove from all services
            for peer_set in state.catchup_requested_peers.values_mut() {
                peer_set.remove(&peer_id);
            }
        }
        // Other events we don't need to handle explicitly
        _ => {}
    }
}

/// Request catch-up from a newly connected peer.
/// Rate-limits requests to avoid overwhelming peers/network.
fn request_catchup_from_peer(
    swarm: &mut Swarm<WavsBehaviour>,
    peer_id: PeerId,
    state: &mut EventLoopState,
) {
    // Request catch-up for each subscribed service (only once per peer per service)
    for service_id in &state.subscribed_services {
        let peer_set = state
            .catchup_requested_peers
            .entry(service_id.clone())
            .or_default();

        // Skip if we've already requested from this peer for this service
        if peer_set.contains(&peer_id) {
            continue;
        }

        // Rate limit: don't send too many concurrent requests per service
        if peer_set.len() >= MAX_CONCURRENT_CATCHUP_REQUESTS_PER_SERVICE {
            tracing::debug!(
                "Skipping catch-up request to {} for service {} (rate limited: {} outstanding)",
                peer_id,
                service_id,
                peer_set.len()
            );
            continue;
        }

        tracing::debug!(
            "Requesting catch-up from {} for service {}",
            peer_id,
            service_id
        );
        let request = CatchUpRequest {
            service_id: service_id.clone(),
        };
        swarm
            .behaviour_mut()
            .catchup
            .send_request(&peer_id, request);

        peer_set.insert(peer_id);
    }
}

/// Handle an incoming catch-up request
fn handle_catchup_request(
    swarm: &mut Swarm<WavsBehaviour>,
    peer: PeerId,
    request: CatchUpRequest,
    channel: request_response::ResponseChannel<CatchUpResponse>,
    state: &EventLoopState,
) {
    tracing::debug!(
        "Received catch-up request from {} for service {}",
        peer,
        request.service_id
    );

    // Check if we're even subscribed to this service
    let is_subscribed = state.subscribed_services.contains(&request.service_id);

    let submissions = state.get_submissions_for_catchup(&request.service_id);
    let count = submissions.len();

    if count > 0 {
        tracing::info!(
            "Catch-up request from {} for service {} (subscribed: {}): returning {} submissions",
            peer,
            request.service_id,
            is_subscribed,
            count
        );
    }

    let response = CatchUpResponse { submissions };

    if let Err(e) = swarm
        .behaviour_mut()
        .catchup
        .send_response(channel, response)
    {
        tracing::warn!("Failed to send catch-up response to {}: {:?}", peer, e);
    } else if count > 0 {
        tracing::debug!(
            "Sent {} submissions in catch-up response to {} for service {}",
            count,
            peer,
            request.service_id
        );
    }
}

/// Handle an incoming catch-up response
///
/// Validates and forwards submissions received during P2P catch-up to the aggregator.
///
/// ## Validation
/// Only processes submissions for services we're actively subscribed to. This prevents:
/// - Processing stale submissions from previous test runs (e2e tests reuse P2P connections)
/// - Wasting resources on services we've unsubscribed from
/// - Race conditions where catch-up delivers submissions before service initialization completes
///
/// ## Retry Interaction
/// Submissions forwarded here may arrive before operators are fully registered on-chain.
/// The aggregator's retry mechanism (see handle_submit_action) handles this by:
/// 1. Detecting SignerNotRegistered errors
/// 2. Saving the queue for retry
/// 3. Automatically retrying when next submission arrives
fn handle_catchup_response(
    peer: PeerId,
    response: CatchUpResponse,
    aggregator_tx: &crossbeam::channel::Sender<AggregatorCommand>,
    state: &EventLoopState,
) {
    if response.submissions.is_empty() {
        tracing::debug!("Received empty catch-up response from {}", peer);
        return;
    }

    tracing::info!(
        "Received catch-up response from {} with {} submissions",
        peer,
        response.submissions.len()
    );

    // Forward each submission to the aggregator, but only for services we're subscribed to
    let mut accepted = 0;
    let mut rejected = 0;
    for submission in response.submissions {
        // Validate: only accept submissions for services we're subscribed to
        let service_id = submission.service_id();
        if !state.subscribed_services.contains(service_id) {
            tracing::warn!(
                "Rejecting catch-up submission from {} for unsubscribed service {}",
                peer,
                service_id
            );
            rejected += 1;
            continue;
        }

        if let Err(e) = aggregator_tx.send(AggregatorCommand::Receive {
            submission,
            peer: Peer::Other(format!("catchup:{}", peer)),
        }) {
            tracing::error!("Failed to send catch-up submission to aggregator: {}", e);
        } else {
            accepted += 1;
        }
    }

    if rejected > 0 {
        tracing::warn!(
            "Catch-up from {}: accepted {} submissions, rejected {} for unsubscribed services",
            peer,
            accepted,
            rejected
        );
    }
}

/// Handle a received gossip message
fn handle_gossip_message(
    propagation_source: PeerId,
    message: gossipsub::Message,
    aggregator_tx: &crossbeam::channel::Sender<AggregatorCommand>,
    state: &mut EventLoopState,
) {
    let submission: Submission = match serde_json::from_slice(&message.data) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                "Failed to deserialize submission from {}: {}",
                propagation_source,
                e
            );
            return;
        }
    };

    // Validate that the submission's service_id matches the topic
    let expected_topic = service_topic_name(submission.service_id());
    if message.topic.as_str() != expected_topic {
        tracing::warn!(
            "Received submission with mismatched service_id from {}: expected topic '{}' but got '{}'",
            propagation_source,
            expected_topic,
            message.topic
        );
        return;
    }

    tracing::info!(
        "Received submission via P2P from {}: {}",
        propagation_source,
        submission.label()
    );

    // Store for catch-up responses
    state.store_submission(submission.clone());

    // Forward to aggregator
    if let Err(e) = aggregator_tx.send(AggregatorCommand::Receive {
        submission,
        peer: Peer::Other(propagation_source.to_string()),
    }) {
        tracing::error!("Failed to send P2P submission to aggregator: {}", e);
    }
}

/// Handle a command from the application
fn handle_command(
    swarm: &mut Swarm<WavsBehaviour>,
    command: P2pCommand,
    state: &mut EventLoopState,
) {
    match command {
        P2pCommand::Publish {
            service_id,
            submission,
        } => {
            let topic_name = service_topic_name(&service_id);
            let topic = IdentTopic::new(&topic_name);

            // Ensure we're subscribed to the topic
            if !state.subscribed_topics.contains(&topic_name) {
                if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                    tracing::error!("Failed to subscribe to topic {}: {}", topic_name, e);
                    return;
                }
                state.subscribed_topics.insert(topic_name.clone());
                state.subscribed_services.insert(service_id.clone());
                tracing::info!("Subscribed to P2P topic: {}", topic_name);
            }

            // Store submission for catch-up responses
            state.store_submission(*submission.clone());

            // Serialize and publish
            let data = match serde_json::to_vec(&*submission) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to serialize submission: {}", e);
                    return;
                }
            };

            match swarm
                .behaviour_mut()
                .gossipsub
                .publish(topic.clone(), data.clone())
            {
                Ok(_) => {
                    tracing::debug!("Published submission to topic {}", topic_name);
                }
                Err(gossipsub::PublishError::NoPeersSubscribedToTopic) => {
                    tracing::debug!("No peers subscribed to {}, queueing for retry", topic_name);
                    // Enforce queue limit to prevent unbounded memory growth during network issues
                    if state.pending_publishes.len() >= MAX_PENDING_PUBLISHES {
                        tracing::warn!(
                            "Pending publishes queue full ({}), dropping oldest entry",
                            MAX_PENDING_PUBLISHES
                        );
                        state.pending_publishes.pop_front();
                    }
                    state.pending_publishes.push_back(PendingPublish {
                        topic_name,
                        data,
                        created_at: Instant::now(),
                        retries: 0,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to publish to topic {}: {}", topic_name, e);
                }
            }
        }
        P2pCommand::Subscribe { service_id } => {
            let topic_name = service_topic_name(&service_id);
            let topic = IdentTopic::new(&topic_name);

            if state.subscribed_topics.contains(&topic_name) {
                return;
            }

            if let Err(e) = swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                tracing::error!("Failed to subscribe to topic {}: {}", topic_name, e);
                return;
            }

            state.subscribed_topics.insert(topic_name.clone());
            state.subscribed_services.insert(service_id.clone());
            tracing::info!("Subscribed to P2P topic: {}", topic_name);

            // Request catch-up from already-connected peers for this new service
            // Rate-limited to avoid overwhelming the network
            let connected_peers: Vec<PeerId> = swarm.connected_peers().cloned().collect();
            for peer_id in connected_peers {
                let peer_set = state
                    .catchup_requested_peers
                    .entry(service_id.clone())
                    .or_default();

                if peer_set.contains(&peer_id) {
                    continue;
                }

                // Rate limit: don't send too many concurrent requests per service
                if peer_set.len() >= MAX_CONCURRENT_CATCHUP_REQUESTS_PER_SERVICE {
                    tracing::debug!(
                        "Skipping catch-up request to {} for service {} (rate limited)",
                        peer_id,
                        service_id
                    );
                    continue;
                }

                tracing::debug!(
                    "Requesting catch-up from {} for newly subscribed service {}",
                    peer_id,
                    service_id
                );
                let request = CatchUpRequest {
                    service_id: service_id.clone(),
                };
                swarm
                    .behaviour_mut()
                    .catchup
                    .send_request(&peer_id, request);
                peer_set.insert(peer_id);
            }
        }
        P2pCommand::Unsubscribe { service_id } => {
            let topic_name = service_topic_name(&service_id);
            let topic = IdentTopic::new(&topic_name);

            if !state.subscribed_topics.contains(&topic_name) {
                tracing::debug!(
                    "Unsubscribe called for topic {} but not subscribed",
                    topic_name
                );
                return;
            }

            if !swarm.behaviour_mut().gossipsub.unsubscribe(&topic) {
                tracing::warn!("Failed to unsubscribe from topic {}", topic_name);
                return;
            }

            state.subscribed_topics.remove(&topic_name);
            state.subscribed_services.remove(&service_id);
            // Also clear stored submissions for this service to free memory
            state.stored_submissions.remove(&service_id);
            // Clear any pending publishes for this topic to avoid wasted retries
            state
                .pending_publishes
                .retain(|p| p.topic_name != topic_name);
            // Clear catch-up state for this service to allow fresh requests on resubscribe
            state.catchup_requested_peers.remove(&service_id);
            tracing::info!("Unsubscribed from P2P topic: {}", topic_name);
        }
        P2pCommand::GetStatus { response_tx } => {
            let local_peer_id = *swarm.local_peer_id();
            let connected_peers: Vec<PeerId> = swarm.connected_peers().cloned().collect();

            // Listen addresses - use tracked addresses from NewListenAddr events
            // These are the actual interface addresses (e.g., 127.0.0.1) rather than
            // the wildcard 0.0.0.0 that was passed to listen_on()
            let listen_addresses: Vec<String> = state
                .listen_addresses
                .iter()
                .map(|addr| format!("{}/p2p/{}", addr, local_peer_id))
                .collect();

            // External addresses - discovered via AutoNAT/Identify, preferred for NAT traversal
            let external_addresses: Vec<String> = swarm
                .external_addresses()
                .map(|addr| format!("{}/p2p/{}", addr, local_peer_id))
                .collect();

            // Get peer counts for each subscribed topic
            let mut topic_peer_counts = HashMap::new();
            for topic_name in state.subscribed_topics.iter() {
                let topic_hash = IdentTopic::new(topic_name).hash();
                let peer_count = swarm.behaviour().gossipsub.mesh_peers(&topic_hash).count();
                topic_peer_counts.insert(topic_name.clone(), peer_count);
            }

            let status = P2pStatus {
                enabled: true,
                local_peer_id: Some(local_peer_id.to_string()),
                listen_addresses,
                external_addresses,
                connected_peers: connected_peers.len(),
                peer_ids: connected_peers.iter().map(|p| p.to_string()).collect(),
                subscribed_topics: state.subscribed_topics.iter().cloned().collect(),
                topic_peer_counts,
            };

            // Ignore send error - the receiver may have been dropped
            let _ = response_tx.send(status);
        }
    }
}

/// Create a GossipSub topic name for a service
fn service_topic_name(service_id: &ServiceId) -> String {
    format!("wavs/{}/topic/v1", service_id)
}

/// Check if a multiaddr is dialable by other peers.
/// Addresses bound to 0.0.0.0 are not dialable because they represent "all interfaces"
/// on the local machine, not a specific IP that external peers can connect to.
fn is_dialable_address(addr: &Multiaddr) -> bool {
    !addr.to_string().contains("/ip4/0.0.0.0/")
}
