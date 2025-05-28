use futures::{Stream, StreamExt};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio_stream::wrappers::IntervalStream;
use utils::telemetry::TriggerMetrics;
use wavs_types::Timestamp;

use crate::trigger_manager::{
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
        let trigger_time = Timestamp::now();
        let lookup_ids = cron_scheduler.lock().unwrap().tick(trigger_time);

        Ok(StreamTriggers::Cron {
            lookup_ids,
            trigger_time,
        })
    }));

    Ok(cron_stream)
}
