use anyhow::{anyhow, bail};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EthereumContractSubmission, Packet, SignerAddress,
};

use crate::http::{
    error::AnyError,
    state::{HttpState, PacketQueue},
};

#[axum::debug_handler]
pub async fn handle_packet(
    State(state): State<HttpState>,
    Json(req): Json<AddPacketRequest>,
) -> impl IntoResponse {
    match inner(state, req.packet).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            tracing::error!("{:?}", e);
            AnyError::from(e).into_response()
        }
    }
}

async fn inner(state: HttpState, packet: Packet) -> anyhow::Result<AddPacketResponse> {
    let event_id = packet.event_id();

    let mut queue = match state.get_packet_queue(&event_id)? {
        PacketQueue::Burned => {
            bail!("Packet queue for event {event_id} is already burned");
        }
        PacketQueue::Alive(queue) => queue,
    };

    // TODO - query operator set from ServiceManager contract
    // it may be some struct, using a Vec as a placeholder for now
    let operator_set = Vec::new();

    validate_packet(&packet, &queue, &operator_set)?;

    let envelope = packet.envelope.clone();
    let block_height = packet.block_height; // See https://github.com/Lay3rLabs/wavs-middleware/issues/54
    let route = packet.route.clone();

    queue.push(packet);

    let count = queue.len();

    // TODO:
    // given the total power of the quorum (which could be, say, 60% of the total operator set power)
    // we need to calculate the power of the signers so far, and see if it meets the quorum power
    // we don't care about count, we care about the power of the signers
    // right now this is just hardcoded for demo purposes
    if count >= 3 {
        let service = state.get_service(&route)?;

        let Aggregator::Ethereum(EthereumContractSubmission {
            chain_name,
            address,
            max_gas,
        }) = service.workflows[&route.workflow_id]
            .aggregator
            .clone()
            .ok_or(anyhow!(
                "No aggregator configured for workflow {} on service {}",
                route.workflow_id,
                route.service_id
            ))?;

        let client = state.get_eth_client(&chain_name).await?;
        let signer_and_signatures = queue
            .drain(..)
            .map(|packet| (packet.signer, packet.signature))
            .collect();
        let tx_receipt = client
            .send_envelope_signatures(
                envelope,
                signer_and_signatures,
                block_height,
                address,
                max_gas,
            )
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
    packet: &Packet,
    queue: &[Packet],
    _operator_set: &[SignerAddress], /* TODO: placeholder */
) -> anyhow::Result<()> {
    // TODO
    // 1. ensure that the signature is valid
    // 2. ensure that the signer is in the operator set
    match queue.first() {
        None => {}
        Some(last_packet) => {
            // check if the packet is the same as the last one
            if packet.envelope != last_packet.envelope {
                bail!("Unexpected envelope difference!");
            }

            // TODO: ensure that the signer is not already in the queue

            // see https://github.com/Lay3rLabs/wavs-middleware/issues/54
            // if packet.block_height != last_packet.block_height {
            //     bail!("Unexpected block height difference!");
            // }
        }
    }

    Ok(())
}
