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

use super::StreamTriggers;

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

        // Convert the results into separate vectors
        let mut lookup_ids = Vec::new();
        let mut trigger_times = Vec::new();
        for (lookup_id, scheduled_time) in trigger_results {
            lookup_ids.push(lookup_id);
            trigger_times.push(scheduled_time);
        }

        Ok(StreamTriggers::Cron {
            lookup_ids,
            scheduled_times: trigger_times,
        })
    }));

    Ok(cron_stream)
}
