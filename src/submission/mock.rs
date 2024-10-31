use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::apis::submission::{ChainMessage, Submission, SubmissionError};
use crate::context::AppContext;

#[derive(Clone)]
pub struct MockSubmission {
    inbox: Arc<Mutex<Vec<ChainMessage>>>,
}

impl MockSubmission {
    const TIMEOUT: Duration = Duration::from_secs(1);
    const POLL: Duration = Duration::from_millis(50);

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inbox: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn received(&self) -> Vec<ChainMessage> {
        self.inbox.lock().unwrap().clone()
    }

    pub fn received_len(&self) -> usize {
        self.inbox.lock().unwrap().len()
    }

    /// This will block until n messages arrive in the inbox, or until 10 seconds passes
    pub fn wait_for_messages(&self, n: usize) -> Result<(), WaitError> {
        self.wait_for_messages_timeout(n, Self::TIMEOUT)
    }

    /// This will block until n messages arrive in the inbox, or until custom Duration passes
    pub fn wait_for_messages_timeout(&self, n: usize, timeout: Duration) -> Result<(), WaitError> {
        let end = Instant::now() + timeout;
        while Instant::now() < end {
            if self.received_len() >= n {
                return Ok(());
            }
            sleep(Self::POLL);
        }
        Err(WaitError::Timeout)
    }
}

#[derive(Error, Debug, PartialEq, Clone)]
pub enum WaitError {
    #[error("Waiting timed out")]
    Timeout,
}

impl Submission for MockSubmission {
    // doing this sync so easier to block on
    // TODO: how to add support for aborting on the kill signal from ctx
    // (Same on mock triggers)
    fn start(&self, ctx: AppContext) -> Result<mpsc::Sender<ChainMessage>, SubmissionError> {
        let (tx, mut rx) = mpsc::channel::<ChainMessage>(10);

        let mock = self.clone();
        ctx.rt.spawn(async move {
            tracing::info!("Submission listening on channel");
            while let Some(msg) = rx.recv().await {
                tracing::info!(
                    "Received message: {} / {}",
                    msg.trigger_data.service_id,
                    msg.trigger_data.workflow_id
                );
                mock.inbox.lock().unwrap().push(msg);
            }
            tracing::info!("Submission channel closed");
        });

        sleep(Duration::from_millis(20));

        Ok(tx)
    }
}

#[cfg(test)]
mod test {
    use std::{thread::sleep, time::Duration};

    use lavs_apis::id::TaskId;

    use crate::apis::{trigger::TriggerData, Trigger, ID};

    use super::*;

    fn dummy_message(service: &str, task_id: u64, payload: &str) -> ChainMessage {
        ChainMessage {
            trigger_data: TriggerData {
                service_id: ID::new(service).unwrap(),
                workflow_id: ID::new(service).unwrap(),
                trigger: Trigger::Queue {
                    task_queue_addr: "task_queue".to_string(),
                    poll_interval: 5,
                },
            },
            task_id: TaskId::new(task_id),
            wasm_result: payload.as_bytes().to_vec(),
            hd_index: 0,
            verifier_addr: "verifier".to_string(),
        }
    }

    #[test]
    fn collect_messages_with_sleep() {
        let submission = MockSubmission::new();
        assert_eq!(submission.received(), vec![]);

        let ctx = AppContext::new();
        let send = submission.start(ctx.clone()).unwrap();

        let msg1 = dummy_message("serv1", 1, "foo");
        let msg2 = dummy_message("serv1", 2, "bar");
        let msg3 = dummy_message("serv1", 3, "baz");

        send.blocking_send(msg1.clone()).unwrap();
        // try waiting a bit. is there a way to block somehow?
        sleep(Duration::from_millis(100));
        assert_eq!(submission.received(), vec![msg1.clone()]);

        send.blocking_send(msg2.clone()).unwrap();
        send.blocking_send(msg3.clone()).unwrap();
        // try waiting a bit. is there a way to block somehow?
        sleep(Duration::from_millis(100));
        assert_eq!(submission.received(), vec![msg1, msg2, msg3]);
    }

    #[test]
    fn collect_messages_with_wait() {
        let submission = MockSubmission::new();
        assert_eq!(submission.received(), vec![]);

        let ctx = AppContext::new();
        let send = submission.start(ctx.clone()).unwrap();

        let msg1 = dummy_message("serv1", 1, "foo");
        let msg2 = dummy_message("serv1", 2, "bar");
        let msg3 = dummy_message("serv1", 3, "baz");

        send.blocking_send(msg1.clone()).unwrap();
        submission.wait_for_messages(1).unwrap();
        assert_eq!(submission.received(), vec![msg1.clone()]);

        send.blocking_send(msg2.clone()).unwrap();
        send.blocking_send(msg3.clone()).unwrap();
        submission.wait_for_messages(3).unwrap();
        assert_eq!(submission.received(), vec![msg1, msg2, msg3]);

        // show this doesn't loop forever if the 4th never appears
        let err = submission
            .wait_for_messages_timeout(4, Duration::from_millis(300))
            .unwrap_err();
        assert_eq!(err, WaitError::Timeout);
    }
}
