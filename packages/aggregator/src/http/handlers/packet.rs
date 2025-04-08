use alloy::primitives::{Address, U256};
use anyhow::{anyhow, bail, ensure};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EthereumContractSubmission, Packet,
};

use crate::http::{
    error::AnyError,
    state::{HttpState, PacketQueue, QueuedPacket},
};

alloy::sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    SimpleServiceManager,
    "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
);

#[axum::debug_handler]
pub async fn handle_packet(
    State(state): State<HttpState>,
    Json(req): Json<AddPacketRequest>,
) -> impl IntoResponse {
    match process_packet(state, req.packet).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            tracing::error!("{:?}", e);
            AnyError::from(e).into_response()
        }
    }
}

async fn process_packet(state: HttpState, packet: Packet) -> anyhow::Result<AddPacketResponse> {
    let event_id = packet.event_id();

    let mut queue = match state.get_packet_queue(&event_id)? {
        PacketQueue::Burned => {
            bail!("Packet queue for event {event_id} is already burned");
        }
        PacketQueue::Alive(queue) => queue,
    };

    let envelope = packet.envelope.clone();
    let block_height = packet.block_height; // See https://github.com/Lay3rLabs/wavs-middleware/issues/54
    let route = packet.route.clone();
    let mut total_weight;
    let threshold;

    // TODO - query operator set from ServiceManager contract
    // it may be some struct, using a Vec as a placeholder for now

    let service = state.get_service(&route)?;
    let aggregator = service.workflows[&route.workflow_id]
        .aggregator
        .as_ref()
        .ok_or(anyhow!(
            "No aggregator configured for workflow {} on service {}",
            route.workflow_id,
            route.service_id
        ))?;

    let packet = match aggregator {
        Aggregator::Ethereum(EthereumContractSubmission { chain_name, .. }) => {
            // this implicitly validates that the signature is valid
            let signer = packet.signature.eth_signer_address(&packet.envelope)?;

            let client = state.get_eth_client(chain_name).await?;
            let service_manager =
                SimpleServiceManager::new(service.manager.eth_address_unchecked(), client.provider);
            let weight = service_manager.getOperatorWeight(signer).call().await?._0;
            total_weight = weight;

            // Sum up weights
            for packet in queue.iter() {
                let weight = service_manager
                    .getOperatorWeight(packet.signer)
                    .call()
                    .await?
                    ._0;
                total_weight = weight
                    .checked_add(total_weight)
                    .ok_or(anyhow!("Total weight calculation overflowed"))?;
            }

            // Get the threshold
            threshold = service_manager
                .getLastCheckpointThresholdWeight()
                .call()
                .await?
                ._0;

            validate_packet(packet, &queue, signer, weight)?
        }
    };

    queue.push(packet);

    let count = queue.len();

    // TODO:
    // given the total power of the quorum (which could be, say, 60% of the total operator set power)
    // we need to calculate the power of the signers so far, and see if it meets the quorum power
    // we don't care about count, we care about the power of the signers
    // right now this is just hardcoded for demo purposes
    if total_weight >= threshold {
        let Aggregator::Ethereum(EthereumContractSubmission {
            chain_name,
            address,
            max_gas,
        }) = aggregator;

        let client = state.get_eth_client(chain_name).await?;
        let signatures = queue
            .drain(..)
            .map(|queued| queued.packet.signature)
            .collect();

        let tx_receipt = client
            .send_envelope_signatures(envelope, signatures, block_height, *address, *max_gas)
            .await?;

        state.save_packet_queue(&event_id, PacketQueue::Burned)?;
        Ok(AddPacketResponse::Sent {
            tx_receipt: Box::new(tx_receipt),
            count,
        })
    } else {
        state.save_packet_queue(&event_id, PacketQueue::Alive(queue))?;
        Ok(AddPacketResponse::Aggregated { count })
    }
}

fn validate_packet(
    packet: Packet,
    queue: &[QueuedPacket],
    signer: Address,
    operator_weight: U256,
) -> anyhow::Result<QueuedPacket> {
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

    Ok(QueuedPacket { packet, signer })
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::{
        primitives::{Bytes, FixedBytes},
        signers::{
            k256::ecdsa::SigningKey,
            local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
            SignerSync,
        },
    };
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
        let queued = validate_packet(packet, &queue, signer_1.address(), U256::ONE).unwrap();
        // got the expected signer address
        assert_eq!(queued.signer, signer_1.address());

        queue.push(queued);

        // "fails" (expectedly) because the signer is the same
        let packet = mock_packet(&signer_1, &envelope_1);
        validate_packet(packet, &queue, signer_1.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the envelope is different
        let packet = mock_packet(&signer_2, &envelope_2);
        validate_packet(packet, &queue, signer_2.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the operator is not registered (0 weight)
        let packet = mock_packet(&signer_2, &envelope_1);
        validate_packet(packet.clone(), &queue, signer_2.address(), U256::ZERO).unwrap_err();

        // passes because the signer is different but envelope is the same
        let queued = validate_packet(packet, &queue, signer_2.address(), U256::ONE).unwrap();
        // got the expected signer address
        assert_eq!(queued.signer, signer_2.address());
        queue.push(queued);
    }

    fn mock_packet(signer: &LocalSigner<SigningKey>, envelope: &Envelope) -> Packet {
        let signature = signer.sign_hash_sync(&envelope.eip191_hash()).unwrap();

        Packet {
            envelope: envelope.clone(),
            block_height: 1,
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
