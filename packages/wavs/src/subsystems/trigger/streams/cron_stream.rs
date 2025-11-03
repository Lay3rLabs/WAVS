use futures::{Stream, StreamExt};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio_stream::wrappers::IntervalStream;
use utils::telemetry::TriggerMetrics;
use wavs_types::Timestamp;

use crate::subsystems::trigger::{
    error::TriggerError,
    schedulers::{cron_scheduler::CronIntervalState, interval_scheduler::IntervalScheduler},
};

use super::{CronHit, StreamTriggers};

pub async fn start_cron_stream(
    cron_scheduler: Arc<Mutex<IntervalScheduler<Timestamp, CronIntervalState>>>,
    _metrics: TriggerMetrics,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamTriggers, TriggerError>> + Send>>, TriggerError>
{
    let interval_stream =
        IntervalStream::new(tokio::time::interval(std::time::Duration::from_secs(1)));

    // Process cron triggers on each interval tick
    let cron_stream = Box::pin(interval_stream.map(move |_| {
        let current_time = Timestamp::now();
        let trigger_results = cron_scheduler.lock().unwrap().tick(current_time);

        // Group triggers by their scheduled timestamp
        let mut time_to_lookup_ids: std::collections::BTreeMap<Timestamp, Vec<_>> =
            std::collections::BTreeMap::new();
        for (lookup_id, scheduled_time) in trigger_results {
            time_to_lookup_ids
                .entry(scheduled_time)
                .or_default()
                .push(lookup_id);
        }

        // Convert the grouped results into CronHit structs
        let hits: Vec<CronHit> = time_to_lookup_ids
            .into_iter()
            .map(|(scheduled_time, lookup_ids)| CronHit {
                lookup_ids,
                scheduled_time,
            })
            .collect();

        Ok(StreamTriggers::Cron { hits })
    }));

    Ok(cron_stream)
}
