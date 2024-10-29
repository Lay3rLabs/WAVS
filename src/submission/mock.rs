use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::apis::submission::{ChainMessage, Submission, SubmissionError};
use crate::context::AppContext;

#[derive(Clone)]
pub struct MockSubmission {
    inbox: Arc<RwLock<Vec<ChainMessage>>>,
}

impl MockSubmission {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inbox: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn received(&self) -> Vec<ChainMessage> {
        self.inbox.blocking_read().clone()
    }
}

impl Submission for MockSubmission {
    // doing this sync so easier to block on
    // TODO: how to add support for aborting on the kill signal from ctx
    // (Same on mock triggers)
    fn start(&self, ctx: AppContext) -> Result<mpsc::Sender<ChainMessage>, SubmissionError> {
        let (tx, mut rx) = mpsc::channel(10);

        let mock = self.clone();
        let mut kill_receiver = ctx.get_kill_receiver();

        ctx.rt.spawn(async move {
            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        Some(msg) => mock.inbox.write().await.push(msg),
                        None => break,
                    },
                    _ = kill_receiver.recv() => break,
                }
            }
        });

        Ok(tx)
    }
}

#[cfg(test)]
mod test {
    use std::{thread::sleep, time::Duration};

    use crate::apis::ID;

    use super::*;

    fn dummy_message(service: &str, task_id: u64, payload: &str) -> ChainMessage {
        ChainMessage {
            service_id: ID::new(service).unwrap(),
            workflow_id: ID::new(service).unwrap(),
            task_id,
            wasm_result: payload.as_bytes().to_vec(),
            hd_index: 0,
            verifier_addr: "verifier".to_string(),
        }
    }

    #[test]
    fn collect_messages() {
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
    fn kill_stops_collection() {
        let submission = MockSubmission::new();
        assert_eq!(submission.received(), vec![]);

        let ctx = AppContext::new();
        let send = submission.start(ctx.clone()).unwrap();

        let msg1 = dummy_message("serv2", 11, "foo");
        let msg2 = dummy_message("serv3", 12, "bar");

        send.blocking_send(msg1.clone()).unwrap();
        // try waiting a bit. is there a way to block somehow?
        sleep(Duration::from_millis(100));
        assert_eq!(submission.received(), vec![msg1.clone()]);

        // now hit the kill switch
        ctx.kill();
        sleep(Duration::from_millis(10));

        // future sends should fail
        send.blocking_send(msg2).unwrap_err();
        // nothing more should show up
        assert_eq!(submission.received(), vec![msg1]);
    }
}
