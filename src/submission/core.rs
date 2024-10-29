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

        ctx.rt.clone().spawn({
            let mut kill_receiver = ctx.get_kill_receiver();
            let _self = self.clone();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::info!("Submissions shutting down");
                    },
                    _ = async move {
                    } => {
                        while let Some(msg) = rx.recv().await {
                            tracing::info!("Received message to submit: {:?}", msg);
                        }

                        tracing::info!("Submission channel closed");
                    }
                }
            }
        });

        Ok(tx)
    }
}
