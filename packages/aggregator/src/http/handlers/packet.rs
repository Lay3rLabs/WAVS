use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use anyhow::{anyhow, bail, ensure};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EthereumContractSubmission, IWavsServiceManager, Packet,
};

use crate::http::{
    error::AnyError,
    state::{HttpState, PacketQueue, QueuedPacket},
};

use tokio::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref QUEUE_MUTEX: Mutex<()> = Mutex::new(());
}

#[utoipa::path(
    post,
    path = "/packet",
    request_body = AddPacketRequest,
    responses(
        (status = 200, description = "Packet successfully added to queue or sent to contract", body = Vec<AddPacketResponse>),
        (status = 400, description = "Invalid packet data or signature"),
        (status = 500, description = "Internal server error during packet processing")
    ),
    description = "Validates and processes a packet, adding it to the aggregation queue. When enough packets from different signers accumulate to meet the threshold, the aggregated packet is sent to the target contract."
)]
#[axum::debug_handler]
pub async fn handle_packet(
    State(state): State<HttpState>,
    Json(req): Json<AddPacketRequest>,
) -> impl IntoResponse {
    match process_packet(state, &req.packet).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            tracing::error!("{:?}", e);
            AnyError::from(e).into_response()
        }
    }
}

async fn process_packet(
    state: HttpState,
    packet: &Packet,
) -> anyhow::Result<Vec<AddPacketResponse>> {
    let event_id = packet.event_id();
    tracing::info!("Processing packet for event ID: {}", event_id);

    // Acquire global lock first before doing anything
    let _lock = QUEUE_MUTEX.lock().await;
    tracing::info!("Acquired lock for event ID: {}", event_id);

    // Get the most current queue state under the lock
    let mut queue = match state.get_packet_queue(&event_id)? {
        PacketQueue::Burned => {
            bail!("Packet queue for event {event_id} is already burned");
        }
        PacketQueue::Alive(queue) => queue,
    };

    // Log actual queue size (for debugging)
    tracing::info!("Found alive queue for event {}, actual size: {}", event_id, queue.len());
    for (i, q) in queue.iter().enumerate() {
        tracing::info!("Queue item {}: signer {}", i, q.signer);
    }

    // Extract signature and validate
    let signer = packet.signature.eth_signer_address(&packet.envelope)?;
    tracing::info!("New packet from signer: {}", signer);

    // Check if already in queue
    let already_in_queue = queue.iter().any(|q| q.signer == signer);
    if already_in_queue {
        tracing::warn!("Signer {} already in queue, ignoring duplicate", signer);

        // Return current state without changing
        let mut responses = Vec::new();
        responses.push(AddPacketResponse::Aggregated { count: queue.len() });
        return Ok(responses);
    }

    let envelope = packet.envelope.clone();
    let route = packet.route.clone();

    // 2. Get all other necessary information
    let service = state.get_service(&route)?;
    let aggregators = &service.workflows[&route.workflow_id].aggregators;

    if aggregators.is_empty() {
        bail!("No aggregator configured for workflow {} on service {}",
              route.workflow_id, route.service_id);
    }

    // 3. Process with each aggregator
    let mut all_sent = true;
    let mut responses: Vec<AddPacketResponse> = Vec::new();

    for (index, aggregator) in aggregators.iter().enumerate() {
        match aggregator {
            Aggregator::Ethereum(EthereumContractSubmission {
                chain_name,
                address,
                max_gas,
            }) => {
                tracing::info!("Using aggregator for chain {} at address {}", chain_name, address);

                // 4. Get client for this chain
                let client = state.get_eth_client(chain_name).await?;
                let service_manager = IWavsServiceManager::new(
                    service.manager.eth_address_unchecked(),
                    client.provider.clone(),
                );

                // 5. Get weights
                let current_weight = service_manager.getOperatorWeight(signer).call().await?;
                tracing::info!("Current signer weight: {}", current_weight);

                // Validate the packet against the queue
                if let Err(e) = validate_packet(packet, &queue, signer, current_weight) {
                    tracing::error!("Packet validation failed: {}", e);
                    return Err(e);
                }

                // 6. Add packet to queue regardless of aggregator index
                tracing::info!("Adding packet from signer {} to queue", signer);
                queue.push(QueuedPacket {
                    packet: packet.clone(),
                    signer,
                });

                // Save updated queue immediately
                tracing::info!("Saving updated queue with {} packets", queue.len());
                state.save_packet_queue(&event_id, PacketQueue::Alive(queue.clone()))?;

                // 7. Calculate total weight across all signers in queue
                let mut total_weight = U256::ZERO;
                for queued_packet in &queue {
                    let weight = service_manager
                        .getOperatorWeight(queued_packet.signer)
                        .call()
                        .await?;
                    tracing::info!("  Queue signer {} weight: {}", queued_packet.signer, weight);
                    total_weight = total_weight.checked_add(weight)
                        .ok_or(anyhow!("Total weight calculation overflowed"))?;
                }
                tracing::info!("Total accumulated weight: {}", total_weight);

                // 8. Get threshold weight
                let threshold = service_manager
                    .getLastCheckpointThresholdWeight()
                    .call()
                    .await?;
                tracing::info!("Threshold weight: {}", threshold);

                // 9. Check if threshold is met
                if total_weight >= threshold {
                    tracing::info!("THRESHOLD MET! total_weight={} >= threshold={}",
                                  total_weight, threshold);

                    // 10. Collect signatures from all queued packets
                    let signatures: Vec<_> = queue
                        .iter()
                        .map(|queued| queued.packet.signature.clone())
                        .collect();
                    tracing::info!("Collected {} signatures for submission", signatures.len());

                    // 11. Submit transaction
                    let block_height = client.provider.get_block_number().await?;

                    tracing::info!("Submitting transaction to contract...");
                    let tx_receipt = client
                        .send_envelope_signatures(
                            envelope.clone(),
                            signatures,
                            block_height,
                            *address,
                            *max_gas,
                        )
                        .await?;

                    if tx_receipt.status() {
                        tracing::info!("Transaction succeeded: {}", tx_receipt.transaction_hash);
                    } else {
                        tracing::error!("Transaction failed: {}", tx_receipt.transaction_hash);
                    }

                    responses.push(AddPacketResponse::Sent {
                        tx_receipt: Box::new(tx_receipt),
                        count: queue.len(),
                    });
                    // any_sent = true;

                    // Mark queue as burned after successful send
                    state.save_packet_queue(&event_id, PacketQueue::Burned)?;
                } else {
                    tracing::info!("Threshold NOT met: total_weight={} < threshold={}",
                                  total_weight, threshold);
                    responses.push(AddPacketResponse::Aggregated { count: queue.len() });
                    // all_sent = false;
                }
            }
        }
    }

    // Return responses for all aggregators
    Ok(responses)
}

fn validate_packet(
    packet: &Packet,
    queue: &[QueuedPacket],
    signer: Address,
    operator_weight: U256,
) -> anyhow::Result<()> {
    tracing::info!("Validating packet from signer: {}", signer);

    match queue.first() {
        None => {
            tracing::info!("Empty queue, no previous packet to compare with");
        }
        Some(prev) => {
            tracing::info!("Comparing with previous packet from signer: {}", prev.signer);

            // Log envelope details for comparison
            tracing::info!("Current envelope eventId: {}", packet.envelope.eventId);
            tracing::info!("Previous envelope eventId: {}", prev.packet.envelope.eventId);
            tracing::info!("Current envelope ordering: {:?}", packet.envelope.ordering);
            tracing::info!("Previous envelope ordering: {:?}", prev.packet.envelope.ordering);

            // Check if payloads are different and why
            if packet.envelope.payload != prev.packet.envelope.payload {
                tracing::info!("Payload comparison failed!");

                // Try to extract and compare as JSON
                if let (Ok(curr), Ok(prev)) = (
                    std::str::from_utf8(&packet.envelope.payload),
                    std::str::from_utf8(&prev.packet.envelope.payload)
                ) {
                    tracing::info!("Current payload (as string): {}", curr);
                    tracing::info!("Previous payload (as string): {}", prev);

                    // Find the different parts
                    for (i, (c1, c2)) in curr.chars().zip(prev.chars()).enumerate() {
                        if c1 != c2 {
                            tracing::info!("First difference at position {}: '{}' vs '{}'", i, c1, c2);
                            break;
                        }
                    }

                    if curr.len() != prev.len() {
                        tracing::info!("Payload length difference: current={}, previous={}",
                            curr.len(), prev.len());
                    }
                }
            }

            // check if the packet is the same as the last one
            if packet.envelope != prev.packet.envelope {
                tracing::error!("Envelope difference detected!");
                bail!("Unexpected envelope difference!");
            }
        }
    }

    for queued_packet in queue {
        if signer == queued_packet.signer {
            tracing::error!("Duplicate signer {} found at position {}", signer, queued_packet.signer);
            bail!("Signer {} already in queue", signer);
        }
    }

    ensure!(!operator_weight.is_zero(), "Operator is not registered");

    // TODO: ensure that the signer is in the operator set

    tracing::info!("Packet validation successful");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::{Bytes, FixedBytes};
    use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
    use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
    use wavs_types::{Envelope, EnvelopeExt, EnvelopeSignature, PacketRoute};

    #[test]
    fn packet_validation() {
        let mut queue = Vec::new();

        let signer_1 = mock_signer();
        let signer_2 = mock_signer();
        let envelope_1 = mock_envelope([1, 2, 3]);
        let envelope_2 = mock_envelope([4, 5, 6]);

        let packet = mock_packet(&signer_1, &envelope_1);

        let derived_signer_1_address = packet
            .signature
            .eth_signer_address(&packet.envelope)
            .unwrap();
        assert_eq!(derived_signer_1_address, signer_1.address());

        // empty queue is okay
        validate_packet(&packet, &queue, signer_1.address(), U256::ONE).unwrap();

        queue.push(QueuedPacket {
            packet: packet.clone(),
            signer: signer_1.address(),
        });

        // "fails" (expectedly) because the signer is the same
        let packet = mock_packet(&signer_1, &envelope_1);
        validate_packet(&packet, &queue, signer_1.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the envelope is different
        let packet = mock_packet(&signer_2, &envelope_2);
        validate_packet(&packet, &queue, signer_2.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the operator is not registered (0 weight)
        let packet = mock_packet(&signer_2, &envelope_1);
        validate_packet(&packet, &queue, signer_2.address(), U256::ZERO).unwrap_err();

        // passes because the signer is different but envelope is the same
        validate_packet(&packet, &queue, signer_2.address(), U256::ONE).unwrap();
        queue.push(QueuedPacket {
            packet: packet.clone(),
            signer: signer_2.address(),
        });
    }

    fn mock_packet(signer: &LocalSigner<SigningKey>, envelope: &Envelope) -> Packet {
        let signature = signer.sign_hash_sync(&envelope.eip191_hash()).unwrap();

        Packet {
            envelope: envelope.clone(),
            route: PacketRoute {
                service_id: "service".parse().unwrap(),
                workflow_id: "workflow".parse().unwrap(),
            },
            signature: EnvelopeSignature::Secp256k1(signature),
        }
    }

    fn mock_signer() -> LocalSigner<SigningKey> {
        MnemonicBuilder::<English>::default()
            .word_count(24)
            .build_random()
            .unwrap()
    }

    fn mock_envelope(payload: impl Into<Bytes>) -> Envelope {
        Envelope {
            payload: payload.into(),
            eventId: FixedBytes([0; 20]),
            ordering: FixedBytes([0; 12]),
        }
    }
}
