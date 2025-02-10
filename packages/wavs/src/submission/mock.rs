use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;
use wavs_types::Submit;

use crate::apis::submission::{ChainMessage, Submission, SubmissionError};
use crate::test_utils::address::rand_address_eth;
use crate::AppContext;

pub fn mock_eigen_submit() -> Submit {
    Submit::eth_contract("eth".try_into().unwrap(), rand_address_eth(), None)
}

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

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum WaitError {
    #[error("Waiting timed out")]
    Timeout,
}

impl Submission for MockSubmission {
    // doing this sync so easier to block on
    // TODO: how to add support for aborting on the kill signal from ctx
    // (Same on mock triggers)
    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "Submission"))]
    fn start(
        &self,
        ctx: AppContext,
        mut rx: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError> {
        let mock = self.clone();
        ctx.rt.spawn(async move {
            tracing::debug!("Submission listening on channel");
            while let Some(msg) = rx.recv().await {
                tracing::debug!("Received message");
                mock.inbox.lock().unwrap().push(msg);
            }
            tracing::debug!("Submission channel closed");
        });

        sleep(Duration::from_millis(20));

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::{thread::sleep, time::Duration};

    use wavs_types::{ChainName, TriggerConfig};

    use crate::test_utils::address::{rand_address_eth, rand_event_eth};

    use super::*;

    fn dummy_message(service: &str, payload: &str) -> ChainMessage {
        ChainMessage {
            trigger_config: TriggerConfig::eth_contract_event(
                service,
                service,
                rand_address_eth(),
                ChainName::new("eth").unwrap(),
                rand_event_eth(),
            )
            .unwrap(),
            wasi_result: payload.as_bytes().to_vec(),
            submit: Submit::eth_contract(ChainName::new("eth").unwrap(), rand_address_eth(), None),
        }
    }

    #[test]
    fn collect_messages_with_sleep() {
        let submission = MockSubmission::new();
        assert_eq!(submission.received(), vec![]);

        let ctx = AppContext::new();

        let (send, rx) = mpsc::channel::<ChainMessage>(2);
        submission.start(ctx.clone(), rx).unwrap();

        let msg1 = dummy_message("serv1", "foo");
        let msg2 = dummy_message("serv1", "bar");
        let msg3 = dummy_message("serv1", "baz");

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
        let (send, rx) = mpsc::channel::<ChainMessage>(2);
        submission.start(ctx.clone(), rx).unwrap();

        let msg1 = dummy_message("serv1", "foo");
        let msg2 = dummy_message("serv1", "bar");
        let msg3 = dummy_message("serv1", "baz");

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
