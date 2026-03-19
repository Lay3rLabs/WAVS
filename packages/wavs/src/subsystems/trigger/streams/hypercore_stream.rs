//! Hypercore trigger stream for WAVS.
//!
//! Opens a hypercore, subscribes to append events, fetches new blocks, and
//! emits `StreamTriggers::Hypercore`. Replication uses Hyperswarm discovery
//! and spawns the replication protocol to ingest data.

use ::hypercore_protocol::discovery_key;
use futures::Stream;
use hypercore::{replication::Event, Hypercore, HypercoreBuilder, PartialKeypair, Storage};
use hyperswarm::{Config as SwarmConfig, Hyperswarm, TopicConfig};
use std::net::SocketAddr;
use std::{path::PathBuf, pin::Pin, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use utils::telemetry::TriggerMetrics;

use crate::subsystems::trigger::error::TriggerError;

use super::{hypercore_protocol, StreamTriggers};

#[derive(Debug, Clone)]
pub struct HypercoreAppendEvent {
    pub feed_key: String,
    pub index: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HypercoreStreamConfig {
    pub storage_dir: PathBuf,
    pub feed_key: String,
    pub hyperswarm_bootstrap: Option<String>,
}

pub async fn start_hypercore_stream(
    config: HypercoreStreamConfig,
    metrics: TriggerMetrics,
    shutdown: tokio::sync::broadcast::Receiver<()>,
) -> Result<
    (
        Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>,
        tokio::sync::oneshot::Receiver<()>,
    ),
    TriggerError,
> {
    std::fs::create_dir_all(&config.storage_dir).map_err(|err| {
        TriggerError::Hypercore(format!(
            "create storage dir {}: {}",
            config.storage_dir.display(),
            err
        ))
    })?;

    let storage = Storage::new_disk(&config.storage_dir, false)
        .await
        .map_err(|err| TriggerError::Hypercore(format!("open storage: {err:?}")))?;

    let (core, feed_key_bytes) = build_core_with_feed_key(storage, &config.feed_key).await?;
    let feed_key = config.feed_key.clone();

    let mut next_index = core.info().length;
    let core = Arc::new(Mutex::new(core));
    let stream_core = Arc::clone(&core);
    let mut receiver = {
        let core = stream_core.lock().await;
        core.event_subscribe()
    };

    let event_stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(event) => match event {
                    Event::Have(have) => {
                        if have.drop {
                            continue;
                        }
                        let end = have.start.saturating_add(have.length);
                        for index in have.start..end {
                            if index < next_index {
                                continue;
                            }
                            let data = {
                                let mut core = stream_core.lock().await;
                                core.get(index).await
                            };
                            match data {
                                Ok(Some(data)) => {
                                    next_index = index.saturating_add(1);
                                    tracing::info!(
                                        "Hypercore append received: index={}, bytes={}",
                                        index,
                                        data.len()
                                    );
                                    yield Ok(StreamTriggers::Hypercore {
                                        event: HypercoreAppendEvent {
                                            feed_key: feed_key.clone(),
                                            index,
                                            data,
                                        },
                                    });
                                }
                                Ok(None) => {
                                    metrics.increment_total_errors("hypercore_missing_block");
                                }
                                Err(err) => {
                                    metrics.increment_total_errors("hypercore_get_error");
                                    yield Err(TriggerError::Hypercore(format!(
                                        "hypercore get {}: {err:?}",
                                        index
                                    )));
                                }
                            }
                        }
                    }
                    Event::DataUpgrade(_) | Event::Get(_) => {}
                },
                Err(err) => {
                    metrics.increment_total_errors("hypercore_event_receive");
                    yield Err(TriggerError::Hypercore(format!(
                        "hypercore event receive: {err:?}"
                    )));
                    break;
                }
            }
        }
    };

    let peer_connected_rx = start_swarm_replication(
        feed_key_bytes,
        Arc::clone(&core),
        shutdown,
        config.hyperswarm_bootstrap.clone(),
    )
    .await?;

    Ok((Box::pin(event_stream), peer_connected_rx))
}

async fn build_core_with_feed_key(
    storage: Storage,
    feed_key_hex: &str,
) -> Result<(Hypercore, [u8; 32]), TriggerError> {
    let feed_key_bytes = const_hex::decode(feed_key_hex.trim())
        .map_err(|err| TriggerError::Hypercore(format!("invalid feed key hex: {err:?}")))?;
    let feed_key: [u8; 32] = feed_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| TriggerError::Hypercore("invalid feed key length".to_string()))?;
    let public = hypercore::VerifyingKey::from_bytes(&feed_key)
        .map_err(|err| TriggerError::Hypercore(format!("invalid feed key: {err:?}")))?;

    let key_pair = PartialKeypair {
        public,
        secret: None,
    };
    let core = HypercoreBuilder::new(storage)
        .key_pair(key_pair)
        .build()
        .await
        .map_err(|err| TriggerError::Hypercore(format!("build hypercore: {err:?}")))?;

    Ok((core, feed_key))
}

async fn start_swarm_replication(
    feed_key: [u8; 32],
    core: Arc<Mutex<Hypercore>>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
    hyperswarm_bootstrap: Option<String>,
) -> Result<tokio::sync::oneshot::Receiver<()>, TriggerError> {
    let topic = discovery_key(&feed_key);

    tracing::info!(
        "Starting hyperswarm replication for feed_key: {}, discovery_key: {:?}",
        const_hex::encode(feed_key),
        topic
    );

    let mut swarm = Hyperswarm::bind(build_swarm_config(hyperswarm_bootstrap.as_deref()))
        .await
        .map_err(|err| TriggerError::Hypercore(format!("bind hyperswarm: {err:?}")))?;

    swarm.configure(topic, TopicConfig::announce_and_lookup());

    tracing::info!(
        "Configured hyperswarm with announce_and_lookup for discovery_key: {:?}",
        topic
    );

    let (replication_ready_tx, replication_ready_rx) = tokio::sync::oneshot::channel::<()>();

    // The hyperswarm DHT only executes announce/lookup commands once after
    // bootstrapping. If no peers have announced yet at that point, the lookup
    // finds nothing and never retries. Work around this by periodically
    // re-issuing announce+lookup via the SwarmHandle while no peers are connected.
    let peer_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let swarm_handle = swarm.handle();
    let relookup_shutdown = shutdown.resubscribe();
    tokio::spawn({
        let peer_count = Arc::clone(&peer_count);
        let mut shutdown = relookup_shutdown;
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.tick().await; // skip immediate first tick
            loop {
                tokio::select! {
                    _ = shutdown.recv() => break,
                    _ = interval.tick() => {
                        if peer_count.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                            continue;
                        }
                        // Clear then re-set to force new announce/lookup
                        swarm_handle.configure(topic, TopicConfig::default());
                        swarm_handle.configure(topic, TopicConfig::announce_and_lookup());
                    }
                }
            }
        }
    });

    // Hyperswarm is async-std based but exposes futures-compatible streams, so it
    // can be polled directly from the tokio runtime that owns hypercore.
    tokio::spawn(async move {
        tracing::info!("Hyperswarm task started, waiting for peer connections...");
        // Only the first peer's replication readiness is signalled.
        let mut replication_ready_tx = Some(replication_ready_tx);

        loop {
            tokio::select! {
                _ = shutdown.recv() => {
                    tracing::info!("Hyperswarm task received shutdown signal");
                    break;
                }
                stream = futures_lite::StreamExt::next(&mut swarm) => {
                    let stream = match stream {
                        Some(Ok(stream)) => stream,
                        Some(Err(err)) => {
                            tracing::warn!("Hyperswarm connection error: {err:?}");
                            continue;
                        }
                        None => {
                            tracing::info!("Hyperswarm stream ended");
                            break;
                        }
                    };

                    peer_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    let peer_addr = stream.peer_addr();
                    tracing::info!(
                        "Hyperswarm connection established (initiator={}, peer_addr={:?})",
                        stream.is_initiator(),
                        peer_addr
                    );

                    let replication_core = Arc::clone(&core);
                    let is_initiator = stream.is_initiator();
                    // Pass the sender only to the first peer's protocol session.
                    let ready_tx = replication_ready_tx.take();
                    let peer_count_for_protocol = Arc::clone(&peer_count);

                    tokio::spawn(async move {
                        if let Err(err) = hypercore_protocol::run_protocol(
                            stream,
                            is_initiator,
                            replication_core,
                            feed_key,
                            ready_tx,
                        )
                        .await
                        {
                            tracing::warn!("Hypercore protocol swarm peer error: {err:?}");
                        }
                        peer_count_for_protocol.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    });
                }
            }
        }
    });

    Ok(replication_ready_rx)
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
