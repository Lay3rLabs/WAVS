//! Hypercore test client for e2e tests.
//!
//! Provides a simple interface to create hypercore feeds, append data,
//! and use hyperswarm for peer discovery during tests.

use ::hypercore_protocol::discovery_key;
use hypercore::{Hypercore, HypercoreBuilder, PartialKeypair, SigningKey, Storage, VerifyingKey};
use hyperswarm::{Config as SwarmConfig, Hyperswarm, TopicConfig};
use rand_core::OsRng;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use wavs::subsystems::trigger::streams::hypercore_protocol;

/// Test client for creating and managing hypercore feeds in e2e tests.
pub struct HypercoreTestClient {
    /// The hypercore feed
    feed: Arc<Mutex<Hypercore>>,
    /// Hex-encoded feed key (public key)
    feed_key: String,
    /// Handle for the hyperswarm task
    _swarm_handle: JoinHandle<()>,
    /// Count of active hyperswarm connections
    connection_count: Arc<AtomicUsize>,
    /// Notifies waiters when a new connection is established
    connection_notify: Arc<Notify>,
    /// TempDir storage - must be kept alive for the lifetime of the client
    _storage_dir: TempDir,
}

impl HypercoreTestClient {
    /// Create a new hypercore feed with a generated keypair.
    ///
    /// This creates a unique temporary storage directory for this test's
    /// hypercore data, which will be automatically cleaned up when the
    /// client is dropped.
    pub async fn new(
        test_name: &str,
        hyperswarm_bootstrap: Option<String>,
    ) -> anyhow::Result<Self> {
        // Create unique tempdir for this test
        let storage_dir = TempDir::new()?;
        let storage_path: PathBuf = storage_dir.path().to_path_buf();

        tracing::info!(
            "Creating hypercore test client for '{}' with storage at: {}",
            test_name,
            storage_path.display()
        );

        // Create hypercore storage
        let storage = Storage::new_disk(&storage_path, false)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create hypercore storage: {e:?}"))?;

        // Generate a new signing keypair with a CSPRNG
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key_bytes = signing_key.verifying_key().to_bytes();
        let feed_key_bytes = public_key_bytes;
        let feed_key = const_hex::encode(public_key_bytes);

        tracing::info!("Generated hypercore feed key: {}", feed_key);

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
            "Hyperswarm configured for discovery key: {}",
            const_hex::encode(topic)
        );

        let feed = Arc::new(Mutex::new(core));
        let swarm_feed = Arc::clone(&feed);
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_notify = Arc::new(Notify::new());

        // Spawn hyperswarm task to handle incoming connections
        let feed_key_for_swarm = feed_key.clone();
        let feed_key_bytes_for_swarm = feed_key_bytes;
        let connection_count_for_swarm = Arc::clone(&connection_count);
        let connection_notify_for_swarm = Arc::clone(&connection_notify);
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
                        connection_count_for_swarm.fetch_add(1, Ordering::SeqCst);
                        connection_notify_for_swarm.notify_waiters();
                        tracing::info!(
                            "Hyperswarm connection established (initiator={}) for feed_key: {}",
                            stream.is_initiator(),
                            feed_key_for_swarm
                        );
                        let feed = Arc::clone(&swarm_feed);
                        let is_initiator = stream.is_initiator();
                        let feed_key_bytes = feed_key_bytes_for_swarm;

                        // Spawn a task for each peer connection
                        tokio::spawn(async move {
                            if let Err(err) = hypercore_protocol::run_protocol(
                                stream,
                                is_initiator,
                                feed,
                                feed_key_bytes,
                            )
                            .await
                            {
                                tracing::warn!("Hypercore protocol swarm peer error: {err:?}");
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!("Hyperswarm connection error: {:?}", err);
                    }
                }
            }

            tracing::info!("Hypercore swarm task ended");
        });

        Ok(Self {
            feed,
            feed_key,
            _swarm_handle: swarm_handle,
            connection_count,
            connection_notify,
            _storage_dir: storage_dir,
        })
    }

    /// Get the hex-encoded feed key (public key).
    ///
    /// This should be used when registering hypercore triggers
    /// in service definitions.
    pub fn feed_key(&self) -> String {
        self.feed_key.clone()
    }

    /// Wait until at least `expected` hyperswarm connections are established.
    pub async fn wait_for_peers(
        &self,
        expected: usize,
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        if expected == 0 {
            return Ok(0);
        }

        let count = tokio::time::timeout(timeout, async {
            loop {
                let current = self.connection_count.load(Ordering::SeqCst);
                if current >= expected {
                    return current;
                }
                self.connection_notify.notified().await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for {expected} hyperswarm peers"))?;

        Ok(count)
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
