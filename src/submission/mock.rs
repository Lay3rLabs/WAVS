use tokio::sync::mpsc;

use crate::{
    apis::submission::{ChainMessage, Submission, SubmissionError},
    context::AppContext,
};

#[derive(Clone)]
pub struct MockSubmission {}

impl MockSubmission {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl Submission for MockSubmission {
    fn start(
        &self,
        ctx: AppContext,
    ) -> Result<mpsc::Sender<ChainMessage>, SubmissionError> {
        let (tx, mut rx) = mpsc::channel(10);

        ctx.rt.spawn(async move {
            loop {
                match rx.recv().await {
                    Some(msg) => {
                        tracing::info!("Received message to submit: {:?}", msg);
                    }
                    None => {
                        tracing::info!("Submission channel closed");
                        break;
                    }
                }
            }
        });

        Ok(tx)
    }
}
