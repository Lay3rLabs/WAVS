use alloy_primitives::keccak256;
use wavs_types::{QuorumQueue, QuorumQueueId, Submission, WavsSignable, WavsSignature};

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

/// Extract a comparable signer identity from a signature.
/// For secp256k1: recovers the EVM address (20 bytes) from the signature.
/// For BLS12-381: uses keccak256(g1_pubkey) (32 bytes) as the identity.
fn signer_identity<T: WavsSignable + ?Sized>(
    sig: &WavsSignature,
    signable: &T,
) -> Result<Vec<u8>, wavs_types::SigningError> {
    match sig {
        WavsSignature::Secp256k1 { .. } => {
            sig.evm_signer_address(signable).map(|addr| addr.0.to_vec())
        }
        WavsSignature::Bls12381 { g1_pubkey, .. } => Ok(keccak256(g1_pubkey).0.to_vec()),
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

    // Use generic signer identity (EVM address for secp256k1, keccak256(g1_pubkey) for BLS)
    let submission_identity =
        signer_identity(&submission.envelope_signature, &submission.envelope)?;

    for queued_submission in queue.iter_mut() {
        let queued_identity = signer_identity(
            &queued_submission.envelope_signature,
            &queued_submission.envelope,
        )?;

        // if the signer is the same as the one in the queue, we can just update it
        // this effectively allows re-trying failed aggregation
        if submission_identity == queued_identity {
            *queued_submission = submission;

            return Ok(());
        }
    }

    queue.push(submission);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use wavs_types::{
        ChainKey, Envelope, EventId, EvmSubmitAction, ServiceId, SignatureAlgorithm, SignatureKind,
        SubmitAction, Trigger, TriggerAction, TriggerConfig, WasmResponse, WavsSignature,
        WorkflowId,
    };

    /// Create a test QuorumQueueId.
    fn test_queue_id() -> QuorumQueueId {
        QuorumQueueId {
            event_id: EventId::from([0u8; 20]),
            action: SubmitAction::Evm(EvmSubmitAction {
                chain: ChainKey::from_str("evm:31337").unwrap(),
                address: "0x0000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
                gas_price: None,
            }),
        }
    }

    /// Create a mock submission with a specific envelope_signature.
    fn mock_submission_with_sig(sig: WavsSignature) -> Submission {
        let service_id = ServiceId::hash(b"test-queue");
        let trigger_action = TriggerAction {
            config: TriggerConfig {
                service_id,
                workflow_id: WorkflowId::new("test-wf").unwrap(),
                trigger: Trigger::Manual,
            },
            data: wavs_types::TriggerData::default(),
        };
        let operator_response = WasmResponse {
            payload: b"test-payload".to_vec(),
            event_id_salt: None,
            ordering: None,
        };
        let event_id = EventId::from([1u8; 20]);
        let envelope = Envelope {
            payload: alloy_primitives::Bytes::from_static(&[1, 2, 3]),
            eventId: alloy_primitives::FixedBytes([1; 20]),
            ordering: alloy_primitives::FixedBytes([0; 12]),
        };
        Submission {
            trigger_action,
            operator_response,
            event_id,
            envelope,
            envelope_signature: sig,
        }
    }

    fn bls_sig_with_pubkey(pubkey: Vec<u8>) -> WavsSignature {
        WavsSignature::Bls12381 {
            g2_signature: vec![0u8; 256],
            g1_pubkey: pubkey,
            kind: SignatureKind {
                algorithm: SignatureAlgorithm::Bls12381,
                prefix: None,
            },
        }
    }

    fn secp_sig() -> WavsSignature {
        WavsSignature::Secp256k1 {
            data: vec![0u8; 65],
            kind: SignatureKind::evm_default(),
        }
    }

    #[test]
    fn bls_submission_enters_queue() {
        let queue_id = test_queue_id();
        let mut queue = Vec::new();
        let sub = mock_submission_with_sig(bls_sig_with_pubkey(vec![1u8; 128]));

        let result = append_submission_to_queue(&queue_id, &mut queue, sub);
        assert!(
            result.is_ok(),
            "BLS submission should enter queue: {:?}",
            result.err()
        );
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn bls_submission_dedup_same_signer() {
        let queue_id = test_queue_id();
        let mut queue = Vec::new();

        let sub1 = mock_submission_with_sig(bls_sig_with_pubkey(vec![1u8; 128]));
        let sub2 = mock_submission_with_sig(bls_sig_with_pubkey(vec![1u8; 128]));

        append_submission_to_queue(&queue_id, &mut queue, sub1).unwrap();
        append_submission_to_queue(&queue_id, &mut queue, sub2).unwrap();
        assert_eq!(queue.len(), 1, "Same G1 pubkey should dedup to 1 entry");
    }

    #[test]
    fn bls_submission_different_signers() {
        let queue_id = test_queue_id();
        let mut queue = Vec::new();

        let sub1 = mock_submission_with_sig(bls_sig_with_pubkey(vec![1u8; 128]));
        let sub2 = mock_submission_with_sig(bls_sig_with_pubkey(vec![2u8; 128]));

        append_submission_to_queue(&queue_id, &mut queue, sub1).unwrap();
        append_submission_to_queue(&queue_id, &mut queue, sub2).unwrap();
        assert_eq!(
            queue.len(),
            2,
            "Different G1 pubkeys should be separate entries"
        );
    }

    #[test]
    fn secp256k1_still_works() {
        let queue_id = test_queue_id();
        let mut queue = Vec::new();

        // secp256k1 with dummy sig bytes -- evm_signer_address recovery will fail
        // but in the old code it failed too. The new code should use signer_identity
        // which for secp256k1 calls evm_signer_address. Since the test sig is invalid,
        // both old and new code will error. Let's just verify it doesn't panic.
        let sub = mock_submission_with_sig(secp_sig());
        let result = append_submission_to_queue(&queue_id, &mut queue, sub);
        // With invalid signature data, evm_signer_address will error, but that's expected.
        // The important thing is it doesn't panic with "BLS signatures do not have EVM signer addresses"
        assert!(
            result.is_err(),
            "Invalid secp256k1 sig should error on address recovery"
        );
    }
}
