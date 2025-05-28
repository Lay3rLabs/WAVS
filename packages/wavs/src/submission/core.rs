use std::{
    collections::{BTreeMap, HashMap},
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use crate::{
    apis::submission::{ChainMessage, Submission, SubmissionError},
    config::Config,
    AppContext,
};
use alloy_provider::Provider;
use alloy_signer_local::PrivateKeySigner;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    config::{AnyChainConfig, EvmChainConfig},
    evm_client::{signing::make_signer, EvmSigningClient},
    telemetry::SubmissionMetrics,
};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    ChainName, Envelope, EnvelopeExt, EvmContractSubmission, Packet, PacketRoute, ServiceID,
    SigningKeyResponse, Submit,
};

#[derive(Clone)]
pub struct CoreSubmission {
    chain_configs: BTreeMap<ChainName, AnyChainConfig>,
    http_client: reqwest::Client,
    // created on-demand from chain_name and hd_index
    evm_signers: Arc<RwLock<HashMap<ServiceID, SignerInfo>>>,
    evm_sending_clients: Arc<RwLock<HashMap<ChainName, EvmSigningClient>>>,
    evm_mnemonic: String,
    evm_mnemonic_hd_index_count: Arc<AtomicU32>,
    metrics: SubmissionMetrics,
}

struct SignerInfo {
    signer: PrivateKeySigner,
    hd_index: u32,
}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    pub fn new(config: &Config, metrics: SubmissionMetrics) -> Result<Self, SubmissionError> {
        Ok(Self {
            chain_configs: config.chains.clone().into(),
            http_client: reqwest::Client::new(),
            evm_signers: Arc::new(RwLock::new(HashMap::new())),
            evm_sending_clients: Arc::new(RwLock::new(HashMap::new())),
            evm_mnemonic: config
                .submission_mnemonic
                .clone()
                .ok_or(SubmissionError::MissingMnemonic)?,
            evm_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
            metrics,
        })
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
    async fn submit_to_evm(
        &self,
        submission: EvmContractSubmission,
        packet: Packet,
    ) -> Result<(), SubmissionError> {
        let EvmContractSubmission {
            chain_name,
            max_gas,
            address,
        } = submission;

        // free up the mutex to add more clients
        let client = {
            self.evm_sending_clients
                .read()
                .unwrap()
                .get(&chain_name)
                .ok_or(SubmissionError::MissingEvmSendingClient(chain_name.clone()))?
                .clone()
        };

        let block_height = client
            .provider
            .get_block_number()
            .await
            .map_err(|e| SubmissionError::FailedToSubmitEvmDirect(e.into()))?;

        let signature_data = packet
            .envelope
            .signature_data(vec![packet.signature], block_height)?;

        let tx_receipt = client
            .send_envelope_signatures(packet.envelope, signature_data, address, max_gas)
            .await
            .map_err(|e| SubmissionError::FailedToSubmitEvmDirect(e.into()))?;

        tracing::info!(
            "Successfully submitted to EVM chain {}: tx_hash={}, service_id={}",
            chain_name,
            tx_receipt.transaction_hash,
            packet.route.service_id
        );

        self.metrics.increment_total_processed_messages("to_evm");

        Ok(())
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

#[async_trait]
impl Submission for CoreSubmission {
    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "Submission"))]
    fn start(
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

                            let ChainMessage {
                                packet_route,
                                envelope,
                                submit
                            } = msg;

                            if matches!(&submit, Submit::None) {
                                tracing::debug!("Skipping submission");
                                continue;
                            }

                            let packet = match _self.make_packet(packet_route, envelope).await {
                                Ok(packet) => packet,
                                Err(e) => {
                                    tracing::error!("Failed to make packet: {:?}", e);
                                    continue;
                                }
                            };

                            match submit {
                                Submit::EvmContract(submission) => {
                                    let _self = _self.clone();
                                    tokio::spawn(
                                        async move {
                                            if let Err(e) = _self.submit_to_evm(submission, packet).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        }
                                    );
                                },
                                Submit::Aggregator{url} => {
                                    if let Err(e) = _self.submit_to_aggregator(url, packet).await {
                                        tracing::error!("{:?}", e);
                                    }
                                }
                                Submit::None => {
                                    tracing::error!("Submit::None here should be unreachable!");
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
    async fn add_service(&self, service: &wavs_types::Service) -> Result<(), SubmissionError> {
        let hd_index = self
            .evm_mnemonic_hd_index_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let signer = make_signer(&self.evm_mnemonic, Some(hd_index))
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

        for workflow in service.workflows.values() {
            if let Submit::EvmContract(EvmContractSubmission { chain_name, .. }) = &workflow.submit
            {
                if !self
                    .evm_sending_clients
                    .read()
                    .unwrap()
                    .contains_key(chain_name)
                {
                    let chain_config: EvmChainConfig = self
                        .chain_configs
                        .get(service.manager.chain_name())
                        .ok_or(SubmissionError::MissingEvmChain)?
                        .clone()
                        .try_into()
                        .map_err(|_| SubmissionError::NotEvmChain)?;

                    let sending_client_config =
                        chain_config.signing_client_config(self.evm_mnemonic.clone())?;

                    let sending_client = EvmSigningClient::new(sending_client_config)
                        .await
                        .map_err(SubmissionError::EVM)?;

                    self.evm_sending_clients
                        .write()
                        .unwrap()
                        .insert(chain_name.clone(), sending_client);
                }
            }
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    fn remove_service(&self, _service_id: ServiceID) -> Result<(), SubmissionError> {
        // nothing we really care about here, it's okay to keep the signing clients in memory
        Ok(())
    }

    fn get_service_key(
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
}
