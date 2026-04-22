use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use alloy_primitives::FixedBytes;
use thiserror::Error;
use utils::{context::AppContext, telemetry::Metrics};
use wavs_types::{EventId, EventOrder, Service, TriggerAction, WasmResponse};

use wavs::{
    config::Config,
    services::Services,
    subsystems::submission::{
        data::{SubmissionRequest, SubmissionRequestDebug},
        SubmissionManager,
    },
};

use crate::wavs_systems::channels::TestChannels;

pub fn mock_event_id() -> EventId {
    FixedBytes::new([1; 20]).into()
}

pub fn mock_event_order() -> EventOrder {
    FixedBytes::new([0; 12]).into()
}

const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(1);
const SUBMISSION_POLL: Duration = Duration::from_millis(50);

/// This will block until n messages arrive in the inbox, or until custom Duration passes
pub fn wait_for_submission_messages(
    submission_manager: &SubmissionManager,
    n: u64,
    duration: Option<Duration>,
) -> Result<(), WaitError> {
    let end = Instant::now() + duration.unwrap_or(SUBMISSION_TIMEOUT);
    while Instant::now() < end {
        if submission_manager.metrics.get_request_count() >= n {
            return Ok(());
        }
        sleep(SUBMISSION_POLL);
    }
    Err(WaitError::Timeout)
}

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum WaitError {
    #[error("Waiting timed out")]
    Timeout,
}

pub fn mock_submission_request(service: &Service, payload: &str) -> SubmissionRequest {
    SubmissionRequest {
        trigger_action: TriggerAction {
            config: wavs_types::TriggerConfig {
                service_id: service.id(),
                workflow_id: service.workflows.keys().next().unwrap().clone(),
                trigger: service.workflows.values().next().unwrap().trigger.clone(),
            },
            data: wavs_types::TriggerData::default(),
        },
        operator_response: WasmResponse {
            payload: payload.as_bytes().to_vec(),
            event_id_salt: None,
            ordering: None,
        },
        service: service.clone(),
        debug: SubmissionRequestDebug {
            do_not_submit_aggregator: true,
        },
    }
}

pub fn mock_submission_manager(
    ctx: AppContext,
    metrics: &Metrics,
    config: &Config,
    channels: &TestChannels,
    services: Services,
) -> SubmissionManager {
    let submission_manager = SubmissionManager::new(
        config,
        metrics.wavs.submission.clone(),
        services,
        channels.dispatcher_to_submission_rx.clone(),
        channels.subsystem_to_dispatcher_tx.clone(),
    )
    .unwrap();

    assert_eq!(submission_manager.metrics.get_request_count(), 0);

    std::thread::spawn({
        let ctx = ctx.clone();
        let submission_manager = submission_manager.clone();
        move || submission_manager.start(ctx)
    });

    submission_manager
}
