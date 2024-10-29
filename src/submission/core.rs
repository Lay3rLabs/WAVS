use tokio::sync::mpsc;

use crate::{
    apis::submission::{ChainMessage, Submission, SubmissionError},
    config::Config,
    context::AppContext,
};

#[derive(Clone)]
pub struct CoreSubmission {}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    pub fn new(_config: &Config) -> Result<Self, SubmissionError> {
        Ok(Self {})
    }
}

impl Submission for CoreSubmission {
    fn start(
        &self,
        ctx: AppContext,
    ) -> Result<mpsc::UnboundedSender<ChainMessage>, SubmissionError> {
        let (tx, mut rx) = mpsc::unbounded_channel();

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
