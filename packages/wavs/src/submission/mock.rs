use alloy_primitives::FixedBytes;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;
use wavs_types::{EventId, EventOrder, ServiceID, Submit};

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

#[async_trait]
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

    async fn add_service(&self, _service: &wavs_types::Service) -> Result<(), SubmissionError> {
        Ok(())
    }

    fn remove_service(&self, _service_id: ServiceID) -> Result<(), SubmissionError> {
        Ok(())
    }

    fn get_service_key(
        &self,
        _service_id: ServiceID,
    ) -> Result<wavs_types::SigningKeyResponse, SubmissionError> {
        Err(SubmissionError::MissingMnemonic)
    }
}

pub fn mock_event_id() -> EventId {
    FixedBytes::new([0; 20]).into()
}

pub fn mock_event_order() -> EventOrder {
    FixedBytes::new([0; 12]).into()
}

#[cfg(test)]
mod test {
    use std::{thread::sleep, time::Duration};

    use wavs_types::{ChainName, Envelope, PacketRoute};

    use crate::test_utils::address::rand_address_eth;

    use super::*;

    fn dummy_message(service: &str, payload: &str) -> ChainMessage {
        ChainMessage {
            packet_route: PacketRoute {
                service_id: service.parse().unwrap(),
                workflow_id: service.parse().unwrap(),
            },
            envelope: Envelope {
                payload: payload.as_bytes().to_vec().into(),
                eventId: mock_event_id().into(),
                ordering: mock_event_order().into(),
            },
            submit: Submit::eth_contract(ChainName::new("eth").unwrap(), rand_address_eth(), None),
        }
    }

    #[test]
    fn collect_messages_with_sleep() {
        let submission = MockSubmission::new();
        assert!(submission.received().is_empty());

        let ctx = AppContext::new();

        let (send, rx) = mpsc::channel::<ChainMessage>(2);
        submission.start(ctx.clone(), rx).unwrap();

        let msg1 = dummy_message("serv1", "foo");
        let msg2 = dummy_message("serv1", "bar");
        let msg3 = dummy_message("serv1", "baz");

        send.blocking_send(msg1.clone()).unwrap();
        // try waiting a bit. is there a way to block somehow?
        sleep(Duration::from_millis(100));
        assert_eq!(submission.received()[0].packet_route, msg1.packet_route);

        send.blocking_send(msg2.clone()).unwrap();
        send.blocking_send(msg3.clone()).unwrap();
        // try waiting a bit. is there a way to block somehow?
        sleep(Duration::from_millis(100));
        assert_eq!(
            submission
                .received()
                .into_iter()
                .map(|x| x.packet_route)
                .collect::<Vec<_>>(),
            vec![msg1.packet_route, msg2.packet_route, msg3.packet_route]
        );
    }

    #[test]
    fn collect_messages_with_wait() {
        let submission = MockSubmission::new();
        assert!(submission.received().is_empty());

        let ctx = AppContext::new();
        let (send, rx) = mpsc::channel::<ChainMessage>(2);
        submission.start(ctx.clone(), rx).unwrap();

        let msg1 = dummy_message("serv1", "foo");
        let msg2 = dummy_message("serv1", "bar");
        let msg3 = dummy_message("serv1", "baz");

        send.blocking_send(msg1.clone()).unwrap();
        submission.wait_for_messages(1).unwrap();
        assert_eq!(submission.received()[0].packet_route, msg1.packet_route);

        send.blocking_send(msg2.clone()).unwrap();
        send.blocking_send(msg3.clone()).unwrap();
        submission.wait_for_messages(3).unwrap();
        assert_eq!(
            submission
                .received()
                .into_iter()
                .map(|x| x.packet_route)
                .collect::<Vec<_>>(),
            vec![msg1.packet_route, msg2.packet_route, msg3.packet_route]
        );

        // show this doesn't loop forever if the 4th never appears
        let err = submission
            .wait_for_messages_timeout(4, Duration::from_millis(300))
            .unwrap_err();
        assert_eq!(err, WaitError::Timeout);
    }
}
