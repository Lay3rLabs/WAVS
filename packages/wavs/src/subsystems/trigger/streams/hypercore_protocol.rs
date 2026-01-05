use anyhow::{Context, Result};
use futures::io::{AsyncRead, AsyncWrite};
use futures::StreamExt;
use hypercore::{Hypercore, RequestBlock, RequestUpgrade};
use hypercore_protocol::schema::{Data, Range, Request, Synchronize};
use hypercore_protocol::{discovery_key, Channel, Event, Message, ProtocolBuilder};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
struct PeerState {
    can_upgrade: bool,
    remote_fork: u64,
    remote_length: u64,
    remote_can_upgrade: bool,
    remote_uploading: bool,
    remote_downloading: bool,
    remote_synced: bool,
    length_acked: u64,
}

impl Default for PeerState {
    fn default() -> Self {
        Self {
            can_upgrade: true,
            remote_fork: 0,
            remote_length: 0,
            remote_can_upgrade: false,
            remote_uploading: true,
            remote_downloading: true,
            remote_synced: false,
            length_acked: 0,
        }
    }
}

pub async fn run_protocol<S>(
    stream: S,
    is_initiator: bool,
    hypercore: Arc<Mutex<Hypercore>>,
    feed_key: [u8; 32],
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let dkey = discovery_key(&feed_key);
    let mut protocol = ProtocolBuilder::new(is_initiator).connect(stream);

    while let Some(event) = protocol.next().await {
        let event = event.context("hypercore protocol event")?;
        match event {
            Event::Handshake(_) => {
                if is_initiator {
                    protocol.open(feed_key).await?;
                }
            }
            Event::DiscoveryKey(key) => {
                if key == dkey {
                    protocol.open(feed_key).await?;
                }
            }
            Event::Channel(channel) => {
                if channel.discovery_key() == &dkey {
                    spawn_peer(channel, hypercore.clone());
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn spawn_peer(mut channel: Channel, hypercore: Arc<Mutex<Hypercore>>) {
    tokio::spawn(async move {
        let mut peer_state = PeerState::default();
        let info = {
            let hypercore = hypercore.lock().await;
            hypercore.info()
        };

        if info.fork != peer_state.remote_fork {
            peer_state.can_upgrade = false;
        }
        let remote_length = if info.fork == peer_state.remote_fork {
            peer_state.remote_length
        } else {
            0
        };

        let sync_msg = Synchronize {
            fork: info.fork,
            length: info.length,
            remote_length,
            can_upgrade: peer_state.can_upgrade,
            uploading: true,
            downloading: true,
        };

        if info.contiguous_length > 0 {
            let range_msg = Range {
                drop: false,
                start: 0,
                length: info.contiguous_length,
            };
            let _ = channel
                .send_batch(&[Message::Synchronize(sync_msg), Message::Range(range_msg)])
                .await;
        } else {
            let _ = channel.send(Message::Synchronize(sync_msg)).await;
        }

        while let Some(message) = channel.next().await {
            if let Err(err) = onmessage(&hypercore, &mut peer_state, &mut channel, message).await {
                tracing::warn!("Hypercore protocol error: {err:?}");
                break;
            }
        }
    });
}

async fn onmessage(
    hypercore: &Arc<Mutex<Hypercore>>,
    peer_state: &mut PeerState,
    channel: &mut Channel,
    message: Message,
) -> Result<()> {
    match message {
        Message::Synchronize(message) => {
            let length_changed = message.length != peer_state.remote_length;
            let first_sync = !peer_state.remote_synced;
            let info = {
                let hypercore = hypercore.lock().await;
                hypercore.info()
            };
            let same_fork = message.fork == info.fork;

            peer_state.remote_fork = message.fork;
            peer_state.remote_length = message.length;
            peer_state.remote_can_upgrade = message.can_upgrade;
            peer_state.remote_uploading = message.uploading;
            peer_state.remote_downloading = message.downloading;
            peer_state.remote_synced = true;

            peer_state.length_acked = if same_fork { message.remote_length } else { 0 };

            let mut messages = Vec::new();

            if first_sync {
                let msg = Synchronize {
                    fork: info.fork,
                    length: info.length,
                    remote_length: peer_state.remote_length,
                    can_upgrade: peer_state.can_upgrade,
                    uploading: true,
                    downloading: true,
                };
                messages.push(Message::Synchronize(msg));
            }

            if peer_state.remote_length > info.length
                && peer_state.length_acked == info.length
                && length_changed
            {
                let msg = Request {
                    id: 1,
                    fork: info.fork,
                    hash: None,
                    block: None,
                    seek: None,
                    upgrade: Some(RequestUpgrade {
                        start: info.length,
                        length: peer_state.remote_length - info.length,
                    }),
                };
                messages.push(Message::Request(msg));
            }

            if !messages.is_empty() {
                channel.send_batch(&messages).await?;
            }
        }
        Message::Request(message) => {
            let (info, proof) = {
                let mut hypercore = hypercore.lock().await;
                let proof = hypercore
                    .create_proof(message.block, message.hash, message.seek, message.upgrade)
                    .await?;
                (hypercore.info(), proof)
            };

            if let Some(proof) = proof {
                let msg = Data {
                    request: message.id,
                    fork: info.fork,
                    hash: proof.hash,
                    block: proof.block,
                    seek: proof.seek,
                    upgrade: proof.upgrade,
                };
                channel.send(Message::Data(msg)).await?;
            }
        }
        Message::Data(message) => {
            let (_old_info, _applied, new_info, request_block) = {
                let mut hypercore = hypercore.lock().await;
                let old_info = hypercore.info();
                let proof = message.clone().into_proof();
                let applied = hypercore.verify_and_apply_proof(&proof).await?;
                let new_info = hypercore.info();
                let request_block: Option<RequestBlock> = if let Some(upgrade) = &message.upgrade {
                    if old_info.length < upgrade.length {
                        let request_index = old_info.length;
                        let nodes = hypercore.missing_nodes(request_index).await?;
                        Some(RequestBlock {
                            index: request_index,
                            nodes,
                        })
                    } else {
                        None
                    }
                } else if let Some(block) = &message.block {
                    if block.index < peer_state.remote_length.saturating_sub(1) {
                        let request_index = block.index + 1;
                        let nodes = hypercore.missing_nodes(request_index).await?;
                        Some(RequestBlock {
                            index: request_index,
                            nodes,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };

                if applied {
                    tracing::debug!(
                        "Hypercore proof applied, length {}",
                        new_info.contiguous_length
                    );
                }
                (old_info, applied, new_info, request_block)
            };

            let mut messages = Vec::new();
            if let Some(upgrade) = &message.upgrade {
                let new_length = upgrade.length;
                let remote_length = if new_info.fork == peer_state.remote_fork {
                    peer_state.remote_length
                } else {
                    0
                };
                messages.push(Message::Synchronize(Synchronize {
                    fork: new_info.fork,
                    length: new_length,
                    remote_length,
                    can_upgrade: false,
                    uploading: true,
                    downloading: true,
                }));
            }
            if let Some(request_block) = request_block {
                messages.push(Message::Request(Request {
                    id: request_block.index + 1,
                    fork: new_info.fork,
                    hash: None,
                    block: Some(request_block),
                    seek: None,
                    upgrade: None,
                }));
            }

            if !messages.is_empty() {
                channel.send_batch(&messages).await?;
            }
        }
        _ => {}
    }
    Ok(())
}
