use anyhow::bail;
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    EthereumContractSubmission, Packet,
};

use crate::http::{
    error::AnyError,
    state::{Destination, HttpState, PacketQueue},
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

    if let Some(last_packet) = queue.first() {
        if packet.envelope.payload != last_packet.envelope.payload {
            bail!("Unexpected envelope difference!");
        }

        if packet.block_height != last_packet.block_height {
            bail!("Unexpected block height difference!");
        }
    }

    let envelope = packet.envelope.clone();
    let block_height = packet.block_height;
    let route = packet.route.clone();

    queue.push(packet);

    let count = queue.len();
    tracing::debug!("Aggregator count: {}", count);

    if count >= state.config.tasks_quorum as usize {
        let destination = state.get_destination(&route)?;
        match destination {
            Destination::Eth(EthereumContractSubmission {
                chain_name,
                address,
                max_gas,
            }) => {
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
            }
        }
    } else {
        state.save_packet_queue(&event_id, PacketQueue::Alive(queue))?;
        Ok(AddPacketResponse::Aggregated { count })
    }
}
