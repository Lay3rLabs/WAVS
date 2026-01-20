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
        let burned_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let _ = tokio::task::spawn_blocking(move || {
            storage
                .quorum_queues
                .insert(id, QuorumQueue::Burned(burned_at))
                .map_err(AggregatorError::Db)
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))?;

        Ok(())
    }

    /// Clean up burned quorum queues that are older than the configured TTL
    pub async fn cleanup_old_burned_queues(&self) -> Result<usize, AggregatorError> {
        let storage = self.storage.clone();
        let ttl_secs = self.config.aggregator.burned_queue_ttl_secs();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        tokio::task::spawn_blocking(move || {
            let mut removed_count = 0;
            let cutoff_time = now.saturating_sub(ttl_secs);

            // Collect keys to remove (can't remove while iterating)
            let keys_to_remove: Vec<QuorumQueueId> = storage
                .quorum_queues
                .iter()
                .filter_map(|entry| {
                    let (key, value) = entry.pair();
                    match value {
                        QuorumQueue::Burned(timestamp) if *timestamp < cutoff_time => {
                            Some(key.clone())
                        }
                        _ => None,
                    }
                })
                .collect();

            // Remove the expired entries
            for key in keys_to_remove {
                storage.quorum_queues.remove(&key);
                removed_count += 1;
            }

            removed_count
        })
        .await
        .map_err(|e| AggregatorError::JoinError(e.to_string()))
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
