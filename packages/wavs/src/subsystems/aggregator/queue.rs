use wavs_types::{QuorumQueue, QuorumQueueId, Submission};

use crate::subsystems::aggregator::{error::AggregatorError, Aggregator};

impl Aggregator {
    pub async fn get_quorum_queue(
        &self,
        id: &QuorumQueueId,
    ) -> Result<QuorumQueue, AggregatorError> {
        let storage = self.storage.clone();

        tokio::task::spawn_blocking({
            let id = id.clone();
            move || {
                storage
                    .quorum_queues
                    .get_cloned(&id)
                    .unwrap_or_else(|| QuorumQueue::Active(Vec::new()))
            }
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))
    }

    #[allow(clippy::result_large_err)]
    pub async fn save_quorum_queue(
        &self,
        id: QuorumQueueId,
        submissions: Vec<Submission>,
    ) -> Result<(), AggregatorError> {
        let storage = self.storage.clone();

        let _ = tokio::task::spawn_blocking(move || {
            storage
                .quorum_queues
                .insert(id, QuorumQueue::Active(submissions))
                .map_err(AggregatorError::Db)
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))?;

        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub async fn burn_quorum_queue(&self, id: QuorumQueueId) -> Result<(), AggregatorError> {
        let storage = self.storage.clone();

        let _ = tokio::task::spawn_blocking(move || {
            storage
                .quorum_queues
                .insert(id, QuorumQueue::Burned)
                .map_err(AggregatorError::Db)
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))?;

        Ok(())
    }
}

pub fn append_submission_to_queue(
    queue_id: &QuorumQueueId,
    queue: &mut Vec<Submission>,
    submission: Submission,
) -> Result<(), AggregatorError> {
    match queue.first() {
        None => {}
        Some(prev) => {
            // check if the submission is the same as the last one
            // TODO - let custom logic here? wasm component?
            if submission.envelope != prev.envelope {
                return Err(AggregatorError::EnvelopeDiff(queue_id.clone()));
            }
        }
    }

    // In addition to extracting for comparison, this also serves to validate the signature
    let submission_signer_address = submission
        .envelope_signature
        .evm_signer_address(&submission.envelope)?;

    for queued_submission in queue.iter_mut() {
        let queued_submission_signer_address = queued_submission
            .envelope_signature
            .evm_signer_address(&queued_submission.envelope)?;

        // if the signer is the same as the one in the queue, we can just update it
        // this effectively allows re-trying failed aggregation
        if submission_signer_address == queued_submission_signer_address {
            *queued_submission = submission;

            return Ok(());
        }
    }

    queue.push(submission);

    Ok(())
}
