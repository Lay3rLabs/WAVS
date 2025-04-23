use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use anyhow::{anyhow, bail, ensure};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EvmContractSubmission, Packet,
};

use crate::http::{
    error::AnyError,
    state::{HttpState, PacketQueue, QueuedPacket},
};

alloy_sol_macro::sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    SimpleServiceManager,
    "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
);

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

    let mut queue = match state.get_packet_queue(&event_id)? {
        PacketQueue::Burned => {
            bail!("Packet queue for event {event_id} is already burned");
        }
        PacketQueue::Alive(queue) => queue,
    };

    let envelope = packet.envelope.clone();
    let route = packet.route.clone();

    let service = state.get_service(&route)?;
    let aggregators = &service.workflows[&route.workflow_id].aggregators;

    if aggregators.is_empty() {
        bail!(
            "No aggregator configured for workflow {} on service {}",
            route.workflow_id,
            route.service_id
        );
    }

    let mut all_sent = true;
    let mut any_sent = false;
    let mut responses: Vec<AddPacketResponse> = Vec::new();

    for (index, aggregator) in aggregators.iter().enumerate() {
        match aggregator {
            Aggregator::Evm(EvmContractSubmission {
                chain_name,
                address,
                max_gas,
            }) => {
                // this implicitly validates that the signature is valid
                let signer = packet.signature.evm_signer_address(&packet.envelope)?;

                let client = state.get_evm_client(chain_name).await?;
                let service_manager = SimpleServiceManager::new(
                    service.manager.evm_address_unchecked(),
                    client.provider.clone(),
                );
                let weight = service_manager.getOperatorWeight(signer).call().await?;
                let mut total_weight = weight;

                // Sum up weights
                for packet in queue.iter() {
                    let weight = service_manager
                        .getOperatorWeight(packet.signer)
                        .call()
                        .await?;
                    total_weight = weight
                        .checked_add(total_weight)
                        .ok_or(anyhow!("Total weight calculation overflowed"))?;
                }

                // Get the threshold
                let threshold = service_manager
                    .getLastCheckpointThresholdWeight()
                    .call()
                    .await?;

                validate_packet(packet, &queue, signer, weight)?;

                if index == 0 {
                    queue.push(QueuedPacket {
                        packet: packet.clone(),
                        signer,
                    });
                }

                // TODO:
                // given the total power of the quorum (which could be, say, 60% of the total operator set power)
                // we need to calculate the power of the signers so far, and see if it meets the quorum power
                // we don't care about count, we care about the power of the signers
                // right now this is just hardcoded for demo purposes
                if total_weight >= threshold {
                    if threshold.is_zero() {
                        tracing::warn!(
                            "you are using threshold of 0 in your AVS quorum, best to only do this for testing"
                        );
                    }

                    let signatures = queue
                        .iter()
                        .map(|queued| queued.packet.signature.clone())
                        .collect();

                    let block_height = client.provider.get_block_number().await?;

                    let tx_receipt = client
                        .send_envelope_signatures(
                            envelope.clone(),
                            signatures,
                            block_height,
                            *address,
                            *max_gas,
                        )
                        .await?;

                    responses.push(AddPacketResponse::Sent {
                        tx_receipt: Box::new(tx_receipt),
                        count: queue.len(),
                    });
                    any_sent = true;
                } else {
                    responses.push(AddPacketResponse::Aggregated { count: queue.len() });
                    all_sent = false;
                }
            }
        }
    }

    // Log warning for mixed state
    if any_sent && !all_sent {
        tracing::warn!("Mixed responses: some packets sent, some aggregated");
    }

    // Apply the state change once, based on tracking variables
    state.save_packet_queue(
        &event_id,
        if all_sent {
            PacketQueue::Burned
        } else {
            PacketQueue::Alive(queue)
        },
    )?;

    Ok(responses)
}

fn validate_packet(
    packet: &Packet,
    queue: &[QueuedPacket],
    signer: Address,
    operator_weight: U256,
) -> anyhow::Result<()> {
    match queue.first() {
        None => {}
        Some(prev) => {
            // check if the packet is the same as the last one
            if packet.envelope != prev.packet.envelope {
                bail!("Unexpected envelope difference!");
            }

            // see https://github.com/Lay3rLabs/wavs-middleware/issues/54
            // if packet.block_height != last_packet.block_height {
            //     bail!("Unexpected block height difference!");
            // }
        }
    }

    for queued_packet in queue {
        if signer == queued_packet.signer {
            bail!("Signer {} already in queue", signer);
        }
    }

    ensure!(!operator_weight.is_zero(), "Operator is not registered");

    // TODO: ensure that the signer is in the operator set

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
            .evm_signer_address(&packet.envelope)
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
