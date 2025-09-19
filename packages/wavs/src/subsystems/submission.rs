pub mod chain_message;
pub mod error;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, AtomicU64},
        Arc, RwLock,
    },
};

use crate::{config::Config, services::Services, tracing_service_info, AppContext};
use alloy_signer_local::PrivateKeySigner;
use chain_message::ChainMessage;
use error::SubmissionError;
use tracing::instrument;
use utils::{evm_client::signing::make_signer, telemetry::SubmissionMetrics};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Credential, Envelope, EnvelopeExt, Packet, ServiceId, SignerResponse, Submit, TriggerData,
    WorkflowId,
};

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SubmissionCommand {
    Kill,
    Submit(ChainMessage),
}

#[derive(Clone)]
pub struct SubmissionManager {
    http_client: reqwest::Client,
    // created on-demand from chain_name and hd_index
    evm_signers: Arc<RwLock<HashMap<ServiceId, SignerInfo>>>,
    evm_mnemonic: Option<Credential>,
    evm_mnemonic_hd_index_count: Arc<AtomicU32>,
    metrics: SubmissionMetrics,
    message_count: Arc<AtomicU64>,
    dispatcher_to_submission_rx: crossbeam::channel::Receiver<SubmissionCommand>,
    #[cfg(debug_assertions)]
    pub debug_packets: Arc<RwLock<Vec<Packet>>>,
    #[cfg(debug_assertions)]
    pub disable_networking: bool,
    pub services: Services,
}

struct SignerInfo {
    signer: PrivateKeySigner,
    hd_index: u32,
}

impl SubmissionManager {
    #[allow(clippy::new_without_default)]
    #[instrument(skip(services), fields(subsys = "Submission"))]
    pub fn new(
        config: &Config,
        metrics: SubmissionMetrics,
        services: Services,
        dispatcher_to_submission_rx: crossbeam::channel::Receiver<SubmissionCommand>,
    ) -> Result<Self, SubmissionError> {
        Ok(Self {
            http_client: reqwest::Client::new(),
            evm_signers: Arc::new(RwLock::new(HashMap::new())),
            evm_mnemonic: config.submission_mnemonic.clone(),
            evm_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
            metrics,
            message_count: Arc::new(AtomicU64::new(0)),
            dispatcher_to_submission_rx,
            #[cfg(debug_assertions)]
            debug_packets: Arc::new(RwLock::new(Vec::new())),
            #[cfg(debug_assertions)]
            disable_networking: config.disable_submission_networking,
            services,
        })
    }

    #[instrument(skip(self, ctx), fields(subsys = "Submission"))]
    pub fn start(&self, ctx: AppContext) {
        while let Ok(msg) = self.dispatcher_to_submission_rx.recv() {
            match msg {
                SubmissionCommand::Kill => {
                    tracing::info!("SubmissionManager received Kill command, shutting down");
                    break;
                }
                SubmissionCommand::Submit(msg) => {
                    let _self = self.clone();
                    ctx.rt.spawn(async move {
                        if let Err(e) = _self.process_message(msg).await {
                            tracing::error!("Error processing message: {:?}", e);
                        }
                    });
                }
            }
        }
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    async fn process_message(&self, msg: ChainMessage) -> Result<(), SubmissionError> {
        let ChainMessage {
            service_id,
            workflow_id,
            envelope,
            submit,
            trigger_data,
            ..
        } = msg;

        if matches!(&submit, Submit::None) {
            tracing::debug!("Skipping submission");
            return Ok(());
        }

        // Check if the service is active
        if !self.services.is_active(&service_id) {
            crate::tracing_service_warn!(
                self.services,
                service_id,
                "Service is not active, skipping message"
            );
            return Ok(());
        }

        self.message_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let packet = self
            .make_packet(
                service_id.clone(),
                workflow_id.clone(),
                envelope,
                trigger_data,
            )
            .await?;

        #[cfg(debug_assertions)]
        {
            self.debug_packets.write().unwrap().push(packet.clone());
        }

        #[cfg(debug_assertions)]
        if self.disable_networking {
            tracing::warn!("Networking is disabled, skipping submission");
            return Ok(());
        }

        match submit {
            Submit::Aggregator { url, .. } => {
                #[cfg(debug_assertions)]
                if msg.debug.do_not_submit_aggregator {
                    tracing::warn!("Test-only flag set, skipping submission to aggregator");
                    return Ok(());
                }

                self.submit_to_aggregator(url, packet).await?;
            }
            Submit::None => {
                if !cfg!(debug_assertions) {
                    tracing::error!("Submit::None here should be unreachable!");
                }
            }
        };

        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    // Adds a service to the submission manager, creating a new signer for it.
    // if no hd_index is provided, it will be automatically assigned.
    pub fn add_service_key(
        &self,
        service_id: ServiceId,
        hd_index: Option<u32>,
    ) -> Result<(), SubmissionError> {
        let hd_index = hd_index.unwrap_or(
            self.evm_mnemonic_hd_index_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        let signer = make_signer(
            self.evm_mnemonic
                .as_ref()
                .ok_or(SubmissionError::MissingMnemonic)?,
            Some(hd_index),
        )
        .map_err(|e| SubmissionError::FailedToCreateEvmSigner(service_id.clone(), e))?;

        tracing::info!(
            "Created new signing client for service {} -> {}",
            service_id,
            signer.address()
        );

        self.evm_signers
            .write()
            .unwrap()
            .insert(service_id, SignerInfo { signer, hd_index });

        Ok(())
    }

    pub fn get_message_count(&self) -> u64 {
        self.message_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[cfg(debug_assertions)]
    pub fn get_debug_packets(&self) -> Vec<Packet> {
        self.debug_packets.read().unwrap().clone()
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub fn get_service_signer(
        &self,
        service_id: ServiceId,
    ) -> Result<SignerResponse, SubmissionError> {
        let key = self
            .evm_signers
            .read()
            .unwrap()
            .get(&service_id)
            .ok_or_else(|| SubmissionError::MissingServiceKey {
                service_id: service_id.clone(),
            })
            .map(
                |SignerInfo { signer, hd_index }| SignerResponse::Secp256k1 {
                    hd_index: *hd_index,
                    evm_address: signer.address().to_string(),
                },
            )?;

        if tracing::enabled!(tracing::Level::INFO) {
            let address = match &key {
                SignerResponse::Secp256k1 { evm_address, .. } => evm_address,
            };

            tracing_service_info!(
                &self.services,
                service_id,
                "Signing key address: {}",
                address
            );
        }

        Ok(key)
    }

    async fn make_packet(
        &self,
        service_id: ServiceId,
        workflow_id: WorkflowId,
        envelope: Envelope,
        trigger_data: TriggerData,
    ) -> Result<Packet, SubmissionError> {
        let evm_signer = {
            let lock = self.evm_signers.read().unwrap();
            lock.get(&service_id)
                .ok_or(SubmissionError::MissingEvmSigner(service_id.clone()))?
                .signer
                .clone()
        };

        let signature_kind = match self
            .services
            .get_workflow(&service_id, &workflow_id)?
            .submit
        {
            Submit::Aggregator { signature_kind, .. } => signature_kind,
            Submit::None => return Err(SubmissionError::InvalidSubmitKind(Submit::None)),
        };

        let signature = envelope
            .sign(&evm_signer, signature_kind)
            .await
            .map_err(SubmissionError::FailedToSignEnvelope)?;

        let service = self.services.get(&service_id)?;

        Ok(Packet {
            service,
            workflow_id,
            envelope,
            signature,
            trigger_data,
        })
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    async fn submit_to_aggregator(
        &self,
        url: String,
        packet: Packet,
    ) -> Result<(), SubmissionError> {
        #[cfg(debug_assertions)]
        if std::env::var("WAVS_FORCE_SUBMISSION_ERROR_XXX").is_ok() {
            self.metrics.submissions_failed.add(1, &[]);
            self.metrics.total_errors.add(1, &[]);
            return Err(SubmissionError::Aggregator(
                "Forced submission error for testing alerts".into(),
            ));
        }

        let service_id = packet.service.id();
        let workflow_id = packet.workflow_id.clone();
        let start_time = std::time::Instant::now();

        #[cfg(debug_assertions)]
        if std::env::var("WAVS_FORCE_SLOW_SUBMISSION_XXX").is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(6));
        }

        let response = self
            .http_client
            .post(format!("{url}/packets"))
            .header("Content-Type", "application/json")
            .json(&AddPacketRequest { packet })
            .send()
            .await
            .map_err(SubmissionError::Reqwest)?;

        if !response.status().is_success() {
            let latency = start_time.elapsed().as_secs_f64();
            self.metrics.record_submission(latency, "aggregator", false);
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
                        "Successfully submitted to aggregator {}: tx_hash={}, payload_count={}, service_id={}, workflow_id={}",
                        url, tx_receipt.transaction_hash, count, service_id, workflow_id
                    );
                }
                AddPacketResponse::Aggregated { count } => {
                    tracing::info!(
                        "Successfully aggregated for service_id={}, workflow_id={}: current_payload_count={}",
                        service_id, workflow_id,
                        count
                    );
                }
                AddPacketResponse::TimerStarted { delay_seconds } => {
                    tracing::info!(
                        "Timer started for service_id={}, workflow_id={}: delay={}s",
                        service_id,
                        workflow_id,
                        delay_seconds
                    );
                }
                AddPacketResponse::Error { reason } => {
                    tracing::error!(
                        "Aggregator errored for service_id={}, workflow_id={}: {}",
                        service_id,
                        workflow_id,
                        reason
                    );
                }
                AddPacketResponse::Burned => {
                    tracing_service_info!(
                        self.services,
                        service_id,
                        "Aggregator queue burned for workflow_id={}",
                        workflow_id
                    );
                }
            }

            self.metrics
                .increment_total_processed_messages("to_aggregator");
        }

        let latency = start_time.elapsed().as_secs_f64();
        self.metrics.record_submission(latency, "aggregator", true);

        Ok(())
    }
}
