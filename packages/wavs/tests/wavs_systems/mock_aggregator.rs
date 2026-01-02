use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use thiserror::Error;
use utils::{context::AppContext, telemetry::Metrics};
use wavs::{config::Config, services::Services, subsystems::aggregator::Aggregator};

use crate::wavs_systems::channels::TestChannels;

pub fn mock_aggregator(
    ctx: AppContext,
    metrics: &Metrics,
    config: &Config,
    channels: &TestChannels,
    services: Services,
) -> Aggregator {
    let aggregator = Aggregator::new(
        &config,
        metrics.wavs.aggregator.clone(),
        services,
        channels.dispatcher_to_aggregator_rx.clone(),
        channels.dispatcher_to_aggregator_tx.clone(),
        channels.subsystem_to_dispatcher_tx.clone(),
    )
    .unwrap();

    assert_eq!(aggregator.metrics.get_broadcast_count(), 0);
    assert_eq!(aggregator.metrics.get_receive_count(), 0);
    assert_eq!(aggregator.metrics.get_execute_count(), 0);

    std::thread::spawn({
        let ctx = ctx.clone();
        let aggregator = aggregator.clone();
        move || aggregator.start(ctx)
    });

    aggregator
}

const AGGREGATOR_TIMEOUT: Duration = Duration::from_secs(1);
const AGGREGATOR_POLL: Duration = Duration::from_millis(50);

pub fn wait_for_aggregator_broadcasts(
    aggregator: &Aggregator,
    n: u64,
    duration: Option<Duration>,
) -> Result<(), WaitError> {
    let end = Instant::now() + duration.unwrap_or(AGGREGATOR_TIMEOUT);
    while Instant::now() < end {
        if aggregator.metrics.get_broadcast_count() >= n {
            return Ok(());
        }
        sleep(AGGREGATOR_POLL);
    }
    Err(WaitError::Timeout)
}

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum WaitError {
    #[error("Waiting timed out")]
    Timeout,
}
