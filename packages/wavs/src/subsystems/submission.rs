pub mod chain_message;
pub mod error;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, AtomicU64},
        Arc, RwLock,
    },
};

use crate::{config::Config, AppContext};
use alloy_signer_local::PrivateKeySigner;
use chain_message::ChainMessage;
use error::SubmissionError;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{evm_client::signing::make_signer, telemetry::SubmissionMetrics};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Envelope, EnvelopeExt, Packet, PacketRoute, ServiceID, SigningKeyResponse, Submit,
};

#[derive(Clone)]
pub struct SubmissionManager {
    http_client: reqwest::Client,
    // created on-demand from chain_name and hd_index
    evm_signers: Arc<RwLock<HashMap<ServiceID, SignerInfo>>>,
    evm_mnemonic: Option<String>,
    evm_mnemonic_hd_index_count: Arc<AtomicU32>,
    metrics: SubmissionMetrics,
    message_count: Arc<AtomicU64>,
    #[cfg(debug_assertions)]
    pub debug_packets: Arc<RwLock<Vec<Packet>>>,
    #[cfg(debug_assertions)]
    pub disable_networking: bool,
}

struct SignerInfo {
    signer: PrivateKeySigner,
    hd_index: u32,
}

impl SubmissionManager {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    pub fn new(config: &Config, metrics: SubmissionMetrics) -> Result<Self, SubmissionError> {
        Ok(Self {
            http_client: reqwest::Client::new(),
            evm_signers: Arc::new(RwLock::new(HashMap::new())),
            evm_mnemonic: config.submission_mnemonic.clone(),
            evm_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
            metrics,
            message_count: Arc::new(AtomicU64::new(0)),
            #[cfg(debug_assertions)]
            debug_packets: Arc::new(RwLock::new(Vec::new())),
            #[cfg(debug_assertions)]
            disable_networking: false,
        })
    }

    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "Submission"))]
    pub fn start(
        &self,
        ctx: AppContext,
        mut rx: mpsc::Receiver<ChainMessage>,
    ) -> Result<(), SubmissionError> {
        ctx.rt.clone().spawn({
            let mut kill_receiver = ctx.get_kill_receiver();
            let _self = self.clone();

            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::debug!("Submissions shutting down");
                    },
                    _ = async move {
                    } => {
                        while let Some(msg) = rx.recv().await {
                            _self.message_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

                            let ChainMessage {
                                packet_route,
                                envelope,
                                submit
                            } = msg;


                            let packet = match _self.make_packet(packet_route, envelope).await {
                                Ok(packet) => packet,
                                Err(e) => {
                                    tracing::error!("Failed to make packet: {:?}", e);
                                    continue;
                                }
                            };

                            #[cfg(debug_assertions)]
                            {
                                _self.debug_packets.write().unwrap().push(packet.clone());
                            }

                            if matches!(&submit, Submit::None) {
                                tracing::debug!("Skipping submission");
                                continue;
                            }

                            #[cfg(debug_assertions)]
                            if _self.disable_networking {
                                tracing::warn!("Networking is disabled, skipping submission");
                                continue;
                            }

                            match submit {
                                Submit::Aggregator{url} => {
                                    if let Err(e) = _self.submit_to_aggregator(url, packet).await {
                                        tracing::error!("{:?}", e);
                                    }
                                }
                                Submit::None => {
                                    if !cfg!(debug_assertions) {
                                        tracing::error!("Submit::None here should be unreachable!");
                                    }
                                }
                            };
                        }
                        tracing::debug!("Submission channel closed");
                    }
                }
            }
        });

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    // Adds a service to the submission manager, creating a new signer for it.
    // if no hd_index is provided, it will be automatically assigned.
    pub fn add_service(&self, service: &wavs_types::Service, hd_index: Option<u32>) -> Result<(), SubmissionError> {
        let hd_index = hd_index.unwrap_or(self
            .evm_mnemonic_hd_index_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst));

        let signer = make_signer(
            self.evm_mnemonic
                .as_ref()
                .ok_or(SubmissionError::MissingMnemonic)?,
            Some(hd_index),
        )
        .map_err(|e| SubmissionError::FailedToCreateEvmSigner(service.id.clone(), e))?;

        tracing::info!(
            "Created new signing client for service {} -> {}",
            service.id,
            signer.address()
        );

        self.evm_signers
            .write()
            .unwrap()
            .insert(service.id.clone(), SignerInfo { signer, hd_index });

        Ok(())
    }

    pub fn get_message_count(&self) -> u64 {
        self.message_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[cfg(debug_assertions)]
    pub fn get_debug_packets(&self) -> Vec<Packet> {
        self.debug_packets.read().unwrap().clone()
    }

    pub fn get_service_key(
        &self,
        service_id: ServiceID,
    ) -> Result<SigningKeyResponse, SubmissionError> {
        self.evm_signers
            .read()
            .unwrap()
            .get(&service_id)
            .ok_or(SubmissionError::MissingMnemonic)
            .map(
                |SignerInfo { signer, hd_index }| SigningKeyResponse::Secp256k1 {
                    hd_index: *hd_index,
                    evm_address: signer.address().to_string(),
                },
            )
    }

    async fn make_packet(
        &self,
        route: PacketRoute,
        envelope: Envelope,
    ) -> Result<Packet, SubmissionError> {
        let evm_signer = {
            let lock = self.evm_signers.read().unwrap();
            lock.get(&route.service_id)
                .ok_or(SubmissionError::MissingEvmSigner(route.service_id.clone()))?
                .signer
                .clone()
        };

        let signature = envelope
            .sign(&evm_signer)
            .await
            .map_err(SubmissionError::FailedToSignEnvelope)?;

        Ok(Packet {
            route,
            envelope,
            signature,
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn submit_to_aggregator(
        &self,
        url: String,
        packet: Packet,
    ) -> Result<(), SubmissionError> {
        let service_id = packet.route.service_id.clone();
        let response = self
            .http_client
            .post(format!("{url}/packet"))
            .header("Content-Type", "application/json")
            .json(&AddPacketRequest { packet })
            .send()
            .await
            .map_err(SubmissionError::Reqwest)?;

        if !response.status().is_success() {
            return Err(SubmissionError::Aggregator(format!(
                "error hitting {url} response: {:?}",
                response
            )));
        }

        let responses: Vec<AddPacketResponse> =
            response.json().await.map_err(SubmissionError::Reqwest)?;

        for response in responses {
            match response {
                AddPacketResponse::Sent { tx_receipt, count } => {
                    tracing::info!(
                        "Successfully submitted to aggregator {}: tx_hash={}, payload_count={}, service_id={}",
                        url, tx_receipt.transaction_hash, count, service_id
                    );
                }
                AddPacketResponse::Aggregated { count } => {
                    tracing::info!(
                        "Successfully aggregated for service_id={}: current_payload_count={}",
                        service_id,
                        count
                    );
                }

                AddPacketResponse::Error { reason } => {
                    tracing::error!(
                        "Aggregator errored for service_id={}: {}",
                        service_id,
                        reason
                    );
                }

                AddPacketResponse::Burned => {
                    tracing::info!("Aggregator queue burned for service_id={}", service_id);
                }
            }

            self.metrics
                .increment_total_processed_messages("to_aggregator");
        }

        Ok(())
    }
}
