//! Hypercore trigger stream for WAVS.
//!
//! Opens a hypercore, subscribes to append events, fetches new blocks, and
//! emits `StreamTriggers::Hypercore`. Requires a replication endpoint and
//! spawns the replication protocol to ingest distributed data.

use futures::Stream;
use hypercore::{replication::Event, Hypercore, HypercoreBuilder, PartialKeypair, Storage};
use std::{path::PathBuf, pin::Pin, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::Mutex;
use tokio_util::compat::TokioAsyncReadCompatExt;
use utils::telemetry::TriggerMetrics;

use crate::subsystems::trigger::error::TriggerError;

use super::{hypercore_protocol, StreamTriggers};

trait ReplicationIo: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite> ReplicationIo for T {}

type ReplicationStream = Box<dyn ReplicationIo + Unpin + Send>;

#[derive(Debug, Clone)]
pub struct HypercoreAppendEvent {
    pub feed_key: String,
    pub index: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HypercoreStreamConfig {
    pub storage_dir: PathBuf,
    pub replication_endpoint: Option<String>,
    pub replication_feed_key: Option<String>,
}

pub async fn start_hypercore_stream(
    config: HypercoreStreamConfig,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    if config.replication_endpoint.is_none() {
        return Err(TriggerError::Hypercore(
            "hypercore replication endpoint is required".to_string(),
        ));
    }

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

    let (core, feed_key, replication_target) =
        build_core_with_replication(storage, &config).await?;

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

    let (endpoint, feed_key_bytes) = replication_target;
    let replication_core = Arc::clone(&core);
    let replication_stream = connect_replication_endpoint(&endpoint).await?.compat();
    tokio::spawn(async move {
        if let Err(err) = hypercore_protocol::run_protocol(
            replication_stream,
            true,
            replication_core,
            feed_key_bytes,
        )
        .await
        {
            tracing::warn!("Hypercore protocol exited: {err:?}");
        }
    });

    Ok(Box::pin(event_stream))
}

async fn build_core_with_replication(
    storage: Storage,
    config: &HypercoreStreamConfig,
) -> Result<(Hypercore, String, (String, [u8; 32])), TriggerError> {
    let endpoint = config.replication_endpoint.clone().ok_or_else(|| {
        TriggerError::Hypercore("hypercore replication endpoint is required".to_string())
    })?;
    let feed_key_hex = config.replication_feed_key.clone().ok_or_else(|| {
        TriggerError::Hypercore(
            "hypercore replication feed key is required when endpoint is set".to_string(),
        )
    })?;
    let feed_key_bytes = const_hex::decode(feed_key_hex.trim())
        .map_err(|err| TriggerError::Hypercore(format!("invalid feed key hex: {err:?}")))?;
    let feed_key: [u8; 32] = feed_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| TriggerError::Hypercore("invalid feed key length".to_string()))?;
    let public = hypercore::VerifyingKey::from_bytes(&feed_key)
        .map_err(|err| TriggerError::Hypercore(format!("invalid feed key: {err:?}")))?;

    let core = HypercoreBuilder::new(storage)
        .key_pair(PartialKeypair {
            public,
            secret: None,
        })
        .build()
        .await
        .map_err(|err| TriggerError::Hypercore(format!("build hypercore: {err:?}")))?;

    Ok((core, feed_key_hex, (endpoint, feed_key)))
}
async fn connect_replication_endpoint(endpoint: &str) -> Result<ReplicationStream, TriggerError> {
    if let Some(path) = endpoint.strip_prefix("unix:") {
        let path = path.trim_start_matches("//");
        let stream = UnixStream::connect(path)
            .await
            .map_err(|err| TriggerError::Hypercore(format!("connect unix peer: {err:?}")))?;
        Ok(Box::new(stream))
    } else {
        let stream = TcpStream::connect(endpoint)
            .await
            .map_err(|err| TriggerError::Hypercore(format!("connect tcp peer: {err:?}")))?;
        Ok(Box::new(stream))
    }
}
