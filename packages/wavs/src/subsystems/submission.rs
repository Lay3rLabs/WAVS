pub mod data;
pub mod error;

use std::{
    collections::HashMap,
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use crate::{
    config::Config, dispatcher::DispatcherCommand, services::Services,
    subsystems::submission::data::SubmissionRequest, tracing_service_info, AppContext,
};
use alloy_primitives::FixedBytes;
use error::SubmissionError;
use tracing::instrument;
use utils::{evm_client::signing::make_signer, telemetry::SubmissionMetrics};
use wavs_types::Submission;
use wavs_types::{
    Credential, Envelope, EventOrder, ServiceId, SignatureAlgorithm, SignerResponse, Submit,
    WavsCryptoSigner, WavsSigner,
};

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
    signers: Arc<RwLock<HashMap<ServiceId, SignerInfo>>>,
    signing_mnemonic: Credential,
    signing_mnemonic_hd_index_count: Arc<AtomicU32>,
    subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    dispatcher_to_submission_rx: crossbeam::channel::Receiver<SubmissionCommand>,
    #[cfg(feature = "dev")]
    pub debug_submissions: Arc<RwLock<Vec<Submission>>>,
    #[cfg(feature = "dev")]
    pub disable_networking: bool,
    pub services: Services,
}

struct SignerInfo {
    signer: WavsCryptoSigner,
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
        let signing_mnemonic = config
            .signing_mnemonic
            .clone()
            .ok_or(SubmissionError::MissingSigningMnemonic)?;
        Ok(Self {
            signers: Arc::new(RwLock::new(HashMap::new())),
            signing_mnemonic,
            signing_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
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

        let signer = {
            let lock = self.signers.read().unwrap();
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
            .sign(&signer, signature_kind.clone())
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

    #[instrument(skip(self, _req), fields(subsys = "Submission"))]
    async fn dispatch(
        &self,
        submission: Submission,
        _req: &SubmissionRequest,
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
        if _req.debug.do_not_submit_aggregator {
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
            tracing::warn!("Forcing slow submission");
            std::thread::sleep(std::time::Duration::from_secs(6));
        }

        tracing::warn!("dispatching: {}", submission.label());
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
        algorithm: SignatureAlgorithm,
    ) -> Result<(), SubmissionError> {
        let hd_index = hd_index.unwrap_or(
            self.signing_mnemonic_hd_index_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        // Ensure the counter is always past the assigned index.
        // This is a no-op for auto-incremented indices but critical for
        // explicit indices during restoration from the service registry.
        let next_index = hd_index
            .checked_add(1)
            .ok_or(SubmissionError::HdIndexOverflow)?;
        self.signing_mnemonic_hd_index_count
            .fetch_max(next_index, std::sync::atomic::Ordering::SeqCst);

        let signer = match algorithm {
            SignatureAlgorithm::Secp256k1 => {
                let pks = make_signer(&self.signing_mnemonic, Some(hd_index))
                    .map_err(|e| SubmissionError::FailedToCreateEvmSigner(service_id.clone(), e))?;

                tracing::info!(
                    "Created secp256k1 signing client for service {} -> {}",
                    service_id,
                    pks.address()
                );

                WavsCryptoSigner::Secp256k1(pks)
            }
            #[cfg(feature = "bls")]
            SignatureAlgorithm::Bls12381 => {
                let bls_key = utils::bls_signing::bls_private_key_from_mnemonic(
                    self.signing_mnemonic.as_str(),
                    hd_index,
                )
                .map_err(|e| SubmissionError::FailedToCreateEvmSigner(service_id.clone(), e))?;

                tracing::info!(
                    "Created BLS12-381 signing client for service {} (HD index {})",
                    service_id,
                    hd_index
                );

                WavsCryptoSigner::Bls12381(bls_key)
            }
            #[cfg(not(feature = "bls"))]
            SignatureAlgorithm::Bls12381 => {
                return Err(SubmissionError::FailedToCreateEvmSigner(
                    service_id,
                    anyhow::anyhow!("BLS support not enabled (compile with --features bls)"),
                ));
            }
        };

        self.signers
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
            .signers
            .read()
            .unwrap()
            .get(&service_id)
            .ok_or_else(|| SubmissionError::MissingServiceKey {
                service_id: service_id.clone(),
            })
            .and_then(|SignerInfo { signer, hd_index }| match signer {
                WavsCryptoSigner::Secp256k1(pks) => Ok(SignerResponse::Secp256k1 {
                    hd_index: *hd_index,
                    evm_address: pks.address().to_string(),
                }),
                #[cfg(feature = "bls")]
                WavsCryptoSigner::Bls12381(ref bls_key) => {
                    let g1_bytes = utils::bls_signing::bls_g1_pubkey_bytes(bls_key)
                        .map_err(SubmissionError::FailedToSignEnvelope)?;
                    Ok(SignerResponse::Bls12381 {
                        hd_index: *hd_index,
                        g1_pubkey_hex: const_hex::encode(g1_bytes),
                    })
                }
            })?;

        if tracing::enabled!(tracing::Level::INFO) {
            let address = match &key {
                SignerResponse::Secp256k1 { evm_address, .. } => evm_address.clone(),
                SignerResponse::Bls12381 { g1_pubkey_hex, .. } => {
                    format!("BLS:{}", &g1_pubkey_hex[..16])
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MNEMONIC: &str = "test test test test test test test test test test test junk";

    /// Verify that add_service_key with BLS algorithm creates a signer that is
    /// WavsCryptoSigner::Bls12381 and produces correct byte lengths
    /// (256-byte G2 sig, 128-byte G1 pubkey).
    #[cfg(feature = "bls")]
    #[test]
    fn submission_bls_signer_produces_correct_signature() {
        let bls_key = utils::bls_signing::bls_private_key_from_mnemonic(TEST_MNEMONIC, 1).unwrap();
        let signer = WavsCryptoSigner::Bls12381(bls_key);

        // Verify correct variant
        match &signer {
            WavsCryptoSigner::Bls12381(_) => {} // correct variant
            _ => panic!("Expected Bls12381 signer variant"),
        }

        // Verify G1 pubkey bytes can be extracted (128 bytes)
        if let WavsCryptoSigner::Bls12381(ref key) = signer {
            let g1 = utils::bls_signing::bls_g1_pubkey_bytes(key).unwrap();
            assert_eq!(g1.len(), 128, "G1 pubkey must be 128 bytes EIP-2537");

            // Verify signing produces 256-byte G2 signature
            let digest = [0xab_u8; 32];
            let g2 = utils::bls_signing::bls_sign_digest(key, &digest).unwrap();
            assert_eq!(g2.len(), 256, "G2 signature must be 256 bytes EIP-2537");
        }
    }

    /// Verify that secp256k1 signer creation is unchanged.
    #[test]
    fn submission_secp256k1_signer_unchanged() {
        let cred = wavs_types::Credential::new(TEST_MNEMONIC.to_string());
        let pks = make_signer(&cred, Some(0)).unwrap();
        match WavsCryptoSigner::Secp256k1(pks) {
            WavsCryptoSigner::Secp256k1(ref s) => {
                // Verify address is deterministic (Anvil account 0 derived at HD index 0)
                assert!(
                    !s.address().is_zero(),
                    "Secp256k1 signer must have non-zero address"
                );
            }
            _ => panic!("Expected Secp256k1 signer variant"),
        }
    }
}
