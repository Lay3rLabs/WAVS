use anyhow::{anyhow, bail};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EthereumContractSubmission, Packet,
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

    // TODO: move this into a method on PacketQueue so we can test etc.
    if let Some(last_packet) = queue.first() {
        if packet.envelope.payload != last_packet.envelope.payload {
            bail!("Unexpected envelope difference!");
        }

        // see https://github.com/Lay3rLabs/wavs-middleware/issues/54
        // if packet.block_height != last_packet.block_height {
        //     bail!("Unexpected block height difference!");
        // }
    }

    // TODO:
    // 1. Ensure that the signer is not already in the queue
    // 2. Ensure that the signature is valid
    // 3. Ensure that the signer is in the operator set  (note - this will be used below for checking quorum satisfaction too)

    let envelope = packet.envelope.clone();
    let block_height = packet.block_height; // related to https://github.com/Lay3rLabs/wavs-middleware/issues/54
    let route = packet.route.clone();

    queue.push(packet);

    let count = queue.len();
    tracing::debug!("Aggregator count: {}", count);

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
