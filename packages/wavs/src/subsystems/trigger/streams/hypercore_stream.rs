use futures::Stream;
use hypercore::{replication::Event, HypercoreBuilder, Storage};
use std::{path::PathBuf, pin::Pin};
use utils::telemetry::TriggerMetrics;

use crate::subsystems::trigger::error::TriggerError;

use super::StreamTriggers;

#[derive(Debug, Clone)]
pub struct HypercoreAppendEvent {
    pub feed_key: String,
    pub index: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HypercoreStreamConfig {
    pub storage_dir: PathBuf,
    pub overwrite: bool,
}

pub async fn start_hypercore_stream(
    config: HypercoreStreamConfig,
    metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    std::fs::create_dir_all(&config.storage_dir).map_err(|err| {
        TriggerError::Hypercore(format!(
            "create storage dir {}: {}",
            config.storage_dir.display(),
            err
        ))
    })?;

    let storage = Storage::new_disk(&config.storage_dir, config.overwrite)
        .await
        .map_err(|err| TriggerError::Hypercore(format!("open storage: {err:?}")))?;

    let mut core = HypercoreBuilder::new(storage)
        .build()
        .await
        .map_err(|err| TriggerError::Hypercore(format!("build hypercore: {err:?}")))?;

    let feed_key = const_hex::encode(core.key_pair().public.to_bytes());
    let mut receiver = core.event_subscribe();
    let mut next_index = core.info().length;

    let stream = async_stream::stream! {
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
                            match core.get(index).await {
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

    Ok(Box::pin(stream))
}
