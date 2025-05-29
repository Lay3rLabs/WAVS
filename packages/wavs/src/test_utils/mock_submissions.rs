use std::{
    thread::sleep,
    time::{Duration, Instant},
};

use alloy_primitives::FixedBytes;
use thiserror::Error;
use wavs_types::{EventId, EventOrder, Submit};

use crate::subsystems::submission::SubmissionManager;

use super::address::rand_address_evm;

pub fn mock_eigen_submit() -> Submit {
    Submit::evm_contract("evm".try_into().unwrap(), rand_address_evm(), None)
}

pub fn mock_event_id() -> EventId {
    FixedBytes::new([0; 20]).into()
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
        if submission_manager.get_message_count() >= n {
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
