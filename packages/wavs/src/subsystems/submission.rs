pub mod data;
pub mod error;

use std::{
    collections::HashMap,
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use crate::{
    config::Config,
    dispatcher::DispatcherCommand,
    services::Services,
    subsystems::submission::data::{Submission, SubmissionRequest},
    tracing_service_info, AppContext,
};
use alloy_primitives::FixedBytes;
use alloy_signer_local::PrivateKeySigner;
use error::SubmissionError;
use tracing::instrument;
use utils::{evm_client::signing::make_signer, telemetry::SubmissionMetrics};
use wavs_types::{Credential, Envelope, EventOrder, ServiceId, SignerResponse, Submit, WavsSigner};

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SubmissionCommand {
    Kill,
    Submit(SubmissionRequest),
}

#[derive(Clone)]
pub struct SubmissionManager {
    // created on-demand from chain_name and hd_index
    pub metrics: SubmissionMetrics,
    evm_signers: Arc<RwLock<HashMap<ServiceId, SignerInfo>>>,
    evm_mnemonic: Option<Credential>,
    evm_mnemonic_hd_index_count: Arc<AtomicU32>,
    subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    dispatcher_to_submission_rx: crossbeam::channel::Receiver<SubmissionCommand>,
    #[cfg(feature = "dev")]
    pub debug_submissions: Arc<RwLock<Vec<Submission>>>,
    #[cfg(feature = "dev")]
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
        subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    ) -> Result<Self, SubmissionError> {
        Ok(Self {
            evm_signers: Arc::new(RwLock::new(HashMap::new())),
            evm_mnemonic: config.submission_mnemonic.clone(),
            evm_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
            metrics,
            subsystem_to_dispatcher_tx,
            dispatcher_to_submission_rx,
            #[cfg(feature = "dev")]
            debug_submissions: Arc::new(RwLock::new(Vec::new())),
            #[cfg(feature = "dev")]
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
                SubmissionCommand::Submit(req) => {
                    let _self = self.clone();
                    ctx.rt.spawn(async move {
                        _self
                            .metrics
                            .increment_request_count(&req.service, req.workflow_id());

                        // Check if the service is active
                        if !_self.services.is_active(req.service_id()) {
                            crate::tracing_service_warn!(
                                _self.services,
                                req.service_id(),
                                "Service is not active, skipping message"
                            );
                            return;
                        }

                        let submission = match _self.sign_request(&req).await {
                            Ok(s) => {
                                _self
                                    .metrics
                                    .increment_sign_count(&req.service, req.workflow_id());
                                s
                            }
                            Err(e) => {
                                _self
                                    .metrics
                                    .increment_sign_error_count(&req.service, req.workflow_id());
                                tracing::error!("Error processing message: {:?}", e);
                                return;
                            }
                        };

                        match _self.dispatch(submission, &req).await {
                            Ok(_) => {
                                _self
                                    .metrics
                                    .increment_dispatch_count(&req.service, req.workflow_id());
                            }
                            Err(e) => {
                                _self.metrics.increment_dispatch_error_count(
                                    &req.service,
                                    req.workflow_id(),
                                );
                                tracing::error!("Error dispatching submission: {:?}", e);
                            }
                        }
                    });
                }
            }
        }
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    pub async fn sign_request(
        &self,
        req: &SubmissionRequest,
    ) -> Result<Submission, SubmissionError> {
        let service_id = req.service_id();

        let event_id = req.event_id().map_err(SubmissionError::EncodeEventId)?;

        let envelope = Envelope {
            // a bit of a heavy clone, but we need it
            payload: req.operator_response.payload.clone().into(),
            eventId: event_id.clone().into(),
            ordering: match req.operator_response.ordering {
                Some(ordering) => EventOrder::new_u64(ordering).into(),
                None => FixedBytes::default(),
            },
        };

        let evm_signer = {
            let lock = self.evm_signers.read().unwrap();
            lock.get(service_id)
                .ok_or(SubmissionError::MissingEvmSigner(service_id.clone()))?
                .signer
                .clone()
        };

        let signature_kind = match self
            .services
            .get_workflow(service_id, req.workflow_id())?
            .submit
        {
            Submit::Aggregator { signature_kind, .. } => signature_kind,
            Submit::None => return Err(SubmissionError::InvalidSubmitKind(Submit::None)),
        };

        let envelope_signature = envelope
            .sign(&evm_signer, signature_kind.clone())
            .await
            .map_err(SubmissionError::FailedToSignEnvelope)?;

        Ok(Submission {
            trigger_action: req.trigger_action.clone(),
            operator_response: req.operator_response.clone(),
            event_id,
            envelope,
            envelope_signature,
        })
    }

    #[instrument(skip(self, req), fields(subsys = "Submission"))]
    async fn dispatch(
        &self,
        submission: Submission,
        req: &SubmissionRequest,
    ) -> Result<(), SubmissionError> {
        #[cfg(feature = "dev")]
        {
            self.debug_submissions
                .write()
                .unwrap()
                .push(submission.clone());
        }

        #[cfg(feature = "dev")]
        if self.disable_networking {
            tracing::warn!("Networking is disabled, skipping submission");
            return Ok(());
        }

        #[cfg(feature = "dev")]
        if req.debug.do_not_submit_aggregator {
            tracing::warn!("Test-only flag set, skipping submission to aggregator");
            return Ok(());
        }

        #[cfg(feature = "dev")]
        if std::env::var("WAVS_FORCE_SUBMISSION_ERROR_XXX").is_ok() {
            return Err(SubmissionError::Aggregator(
                "Forced submission error for testing alerts".into(),
            ));
        }

        #[cfg(feature = "dev")]
        if std::env::var("WAVS_FORCE_SLOW_SUBMISSION_XXX").is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(6));
        }

        self.subsystem_to_dispatcher_tx
            .send(DispatcherCommand::SubmissionResponse(submission))
            .map_err(Box::new)?;

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

    #[cfg(feature = "dev")]
    pub fn get_debug_submissions(&self) -> Vec<Submission> {
        self.debug_submissions.read().unwrap().clone()
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
}
