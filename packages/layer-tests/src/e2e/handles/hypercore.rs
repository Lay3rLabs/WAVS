//! Hypercore test client for e2e tests.
//!
//! Provides a simple interface to create hypercore feeds, append data,
//! and use hyperswarm for peer discovery during tests.

use ::hypercore_protocol::discovery_key;
use hypercore::{Hypercore, HypercoreBuilder, PartialKeypair, SigningKey, Storage, VerifyingKey};
use hyperswarm::{Config as SwarmConfig, Hyperswarm, TopicConfig};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use wavs::subsystems::trigger::streams::hypercore_protocol;

/// Test client for creating and managing hypercore feeds in e2e tests.
pub struct HypercoreTestClient {
    /// The hypercore feed
    feed: Arc<Mutex<Hypercore>>,
    /// Hex-encoded feed key (public key)
    feed_key: String,
    /// Handle for the hyperswarm task
    swarm_handle: JoinHandle<()>,
    /// TempDir storage - must be kept alive for the lifetime of the client
    _storage_dir: TempDir,
    /// Connection count for testing mesh formation
    connection_count_for_swarm: Arc<AtomicUsize>,
}

// Properly clean up the swarm task when the client is dropped
impl Drop for HypercoreTestClient {
    fn drop(&mut self) {
        tracing::info!(
            "Dropping HypercoreTestClient for feed_key: {}, aborting swarm task",
            self.feed_key
        );
        self.swarm_handle.abort();
    }
}

impl HypercoreTestClient {
    /// Create a new hypercore feed with a pre-generated signing key.
    ///
    /// This is used when we need the feed_key early (for trigger registration)
    /// but want to delay creating the full client until services are ready.
    pub async fn new(
        test_name: &str,
        hyperswarm_bootstrap: Option<String>,
        signing_key_bytes: &[u8],
    ) -> anyhow::Result<Self> {
        // Create unique tempdir for this test
        let storage_dir = TempDir::new()?;
        let storage_path: PathBuf = storage_dir.path().to_path_buf();

        tracing::info!(
            "Creating hypercore test client with pre-generated key for '{}' with storage at: {}",
            test_name,
            storage_path.display()
        );

        // Create hypercore storage
        let storage = Storage::new_disk(&storage_path, false)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create hypercore storage: {e:?}"))?;

        // Reconstruct the signing key from bytes
        // Convert slice to array for SigningKey::from_bytes
        let key_array: [u8; 32] = signing_key_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signing key length, expected 32 bytes"))?;
        let signing_key = SigningKey::from_bytes(&key_array);

        let public_key_bytes = signing_key.verifying_key().to_bytes();
        let feed_key_bytes = public_key_bytes;
        let feed_key = const_hex::encode(public_key_bytes);

        tracing::info!("Using hypercore feed key: {}", feed_key);

        // Reconstruct VerifyingKey from bytes for owned value
        let public = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to create verifying key: {e:?}"))?;

        // Create a PartialKeypair with both public and secret keys (for writable feed)
        let key_pair = PartialKeypair {
            public,
            secret: Some(signing_key),
        };

        // Build hypercore with the generated keypair
        let core = HypercoreBuilder::new(storage)
            .key_pair(key_pair)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build hypercore: {e:?}"))?;

        // Set up hyperswarm for peer discovery
        let topic = discovery_key(&public_key_bytes);

        let mut swarm = Hyperswarm::bind(build_swarm_config(hyperswarm_bootstrap.as_deref()))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind hyperswarm: {e:?}"))?;

        // Announce and lookup for this feed's discovery key
        swarm.configure(topic, TopicConfig::announce_and_lookup());

        tracing::info!(
            "Hyperswarm configured for discovery key: {}, topic: {:?}",
            const_hex::encode(topic),
            topic
        );

        let feed = Arc::new(Mutex::new(core));
        let swarm_feed = Arc::clone(&feed);

        // Spawn hyperswarm task to handle incoming connections
        let feed_key_for_swarm = feed_key.clone();
        let feed_key_bytes_for_swarm = feed_key_bytes;
        let connection_count_for_swarm = Arc::new(AtomicUsize::new(0));

        // Clone the Arc for the spawned task (we keep the original for the struct)
        let swarm_connection_count = Arc::clone(&connection_count_for_swarm);
        let swarm_handle = tokio::spawn(async move {
            let mut swarm = swarm;
            tracing::info!(
                "Hypercore swarm task started, listening for peers for feed_key: {}",
                feed_key_for_swarm
            );

            use futures_lite::StreamExt;
            while let Some(result) = swarm.next().await {
                match result {
                    Ok(stream) => {
                        swarm_connection_count.fetch_add(1, Ordering::SeqCst);
                        tracing::debug!(
                            "Hyperswarm peer discovery attempt (initiator={}, peer_addr={:?}) for feed_key: {}",
                            stream.is_initiator(),
                            stream.peer_addr(),
                            feed_key_for_swarm
                        );
                        tracing::info!(
                            "Hyperswarm connection established (initiator={}, peer_addr={:?}) for feed_key: {}",
                            stream.is_initiator(),
                            stream.peer_addr(),
                            feed_key_for_swarm
                        );
                        let feed = Arc::clone(&swarm_feed);
                        let is_initiator = stream.is_initiator();
                        let feed_key_bytes = feed_key_bytes_for_swarm;

                        // Spawn a task for each peer connection
                        let connection_count_for_peer = Arc::clone(&swarm_connection_count);
                        tokio::spawn(async move {
                            let result = hypercore_protocol::run_protocol(
                                stream,
                                is_initiator,
                                feed,
                                feed_key_bytes,
                            )
                            .await;

                            // Decrement connection count when peer connection closes
                            connection_count_for_peer.fetch_sub(1, Ordering::SeqCst);

                            if let Err(err) = result {
                                tracing::error!(
                                    "Hyperswarm connection failed for feed_key {}: {:?}",
                                    const_hex::encode(feed_key_bytes),
                                    err
                                );
                            } else {
                                tracing::debug!(
                                    "Hypercore protocol peer connection closed cleanly"
                                );
                            }
                        });
                    }
                    Err(err) => {
                        tracing::error!(
                            "Hyperswarm connection failed for feed_key {}: {:?}",
                            feed_key_for_swarm,
                            err
                        );
                    }
                }
            }

            tracing::info!("Hypercore swarm task ended");
        });

        Ok(Self {
            feed,
            feed_key,
            swarm_handle,
            _storage_dir: storage_dir,
            connection_count_for_swarm,
        })
    }

    /// Get the hex-encoded feed key (public key).
    ///
    /// This should be used when registering hypercore triggers
    /// in service definitions.
    pub fn feed_key(&self) -> String {
        self.feed_key.clone()
    }

    /// Get the current number of connected peers for this hypercore feed.
    ///
    /// This is used in tests to wait for mesh formation before proceeding.
    pub fn connected_peer_count(&self) -> usize {
        self.connection_count_for_swarm
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Append data to the hypercore feed.
    ///
    /// Returns the index of the appended block.
    pub async fn append(&self, data: Vec<u8>) -> anyhow::Result<u64> {
        let mut feed = self.feed.lock().await;
        let outcome = feed
            .append(&data)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to append to hypercore: {e:?}"))?;

        // AppendOutcome contains the length, we need to return the index
        let index = outcome.length.saturating_sub(1);

        tracing::info!(
            "Appended {} bytes to hypercore feed at index {}",
            data.len(),
            index
        );

        Ok(index)
    }
}

fn build_swarm_config(hyperswarm_bootstrap: Option<&str>) -> SwarmConfig {
    if let Some(addr) = hyperswarm_bootstrap {
        match addr.parse::<SocketAddr>() {
            Ok(addr) => {
                tracing::info!("Using hyperswarm bootstrap: {}", addr);
                return SwarmConfig::default()
                    .set_bootstrap_nodes(&[addr])
                    .with_defaults();
            }
            Err(err) => {
                tracing::warn!("Invalid hyperswarm bootstrap address '{}': {err}", addr);
            }
        }
    }

    SwarmConfig::all()
}
