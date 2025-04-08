use std::{
    collections::{BTreeMap, HashMap},
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use crate::{
    apis::submission::{ChainMessage, Submission, SubmissionError},
    config::Config,
    AppContext,
};
use alloy::providers::Provider;
use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    config::{AnyChainConfig, EthereumChainConfig},
    eth_client::{EthClientBuilder, EthClientTransport, EthSigningClient},
};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    ChainName, Envelope, EthereumContractSubmission, Packet, PacketRoute, ServiceID,
    SigningKeyResponse, Submit,
};

#[derive(Clone)]
pub struct CoreSubmission {
    chain_configs: BTreeMap<ChainName, AnyChainConfig>,
    http_client: reqwest::Client,
    // created on-demand from chain_name and hd_index
    eth_signing_clients: Arc<RwLock<HashMap<ServiceID, EthSigningClient>>>,
    eth_sending_clients: Arc<RwLock<HashMap<ChainName, EthSigningClient>>>,
    eth_mnemonic: String,
    eth_mnemonic_hd_index_count: Arc<AtomicU32>,
}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    pub fn new(config: &Config) -> Result<Self, SubmissionError> {
        Ok(Self {
            chain_configs: config.chains.clone().into(),
            http_client: reqwest::Client::new(),
            eth_signing_clients: Arc::new(RwLock::new(HashMap::new())),
            eth_sending_clients: Arc::new(RwLock::new(HashMap::new())),
            eth_mnemonic: config
                .submission_mnemonic
                .clone()
                .ok_or(SubmissionError::MissingMnemonic)?,
            eth_mnemonic_hd_index_count: Arc::new(AtomicU32::new(1)),
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn maybe_tap_eth_faucet(
        &self,
        chain_name: ChainName,
        client: &EthSigningClient,
    ) -> Result<(), SubmissionError> {
        let chain_config = self
            .chain_configs
            .get(&chain_name)
            .ok_or(SubmissionError::MissingEthereumChain)?;
        let chain_config: EthereumChainConfig = chain_config
            .clone()
            .try_into()
            .map_err(|_| SubmissionError::MissingEthereumChain)?;

        let _faucet_url = match chain_config.faucet_endpoint.clone() {
            Some(url) => url,
            None => {
                tracing::debug!("No faucet configured, skipping");
                return Ok(());
            }
        };

        todo!()
    }

    async fn make_packet(
        &self,
        route: PacketRoute,
        envelope: Envelope,
    ) -> Result<Packet, SubmissionError> {
        let eth_client = {
            let lock = self.eth_signing_clients.read().unwrap();
            lock.get(&route.service_id)
                .ok_or(SubmissionError::MissingEthereumSigningClient(
                    route.service_id.clone(),
                ))?
                .clone()
        };

        let block_height = eth_client
            .provider
            .get_block_number()
            .await
            .map_err(|e| SubmissionError::Ethereum(anyhow!("{}", e)))?
            - 1;

        let signature = eth_client.sign_envelope(&envelope).await?;

        Ok(Packet {
            route,
            envelope,
            signature,
            block_height,
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn submit_to_ethereum(
        &self,
        submission: EthereumContractSubmission,
        packet: Packet,
    ) -> Result<(), SubmissionError> {
        let EthereumContractSubmission {
            chain_name,
            max_gas,
            address,
        } = submission;

        let eth_client = {
            let lock = self.eth_sending_clients.read().unwrap();
            lock.get(&chain_name)
                .ok_or(SubmissionError::MissingEthereumSendingClient(
                    chain_name.clone(),
                ))?
                .clone()
        };

        if let Err(err) = self.maybe_tap_eth_faucet(chain_name, &eth_client).await {
            tracing::error!(
                "Failed to tap faucet for client {}: {:?}",
                eth_client.address(),
                err
            );
        }

        let _tx_receipt = eth_client
            .send_envelope_signatures(
                packet.envelope,
                vec![packet.signature],
                packet.block_height,
                address,
                max_gas,
            )
            .await
            .map_err(|e| SubmissionError::FailedToSubmitEthDirect(e.into()))?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn submit_to_aggregator(
        &self,
        url: String,
        packet: Packet,
    ) -> Result<(), SubmissionError> {
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

        let response: AddPacketResponse =
            response.json().await.map_err(SubmissionError::Reqwest)?;

        match response {
            AddPacketResponse::Sent { tx_receipt, count } => {
                tracing::debug!(
                    "Aggregator submitted with tx hash {} and payload count {}",
                    tx_receipt.transaction_hash,
                    count
                );
            }
            AddPacketResponse::Aggregated { count } => {
                tracing::debug!("Aggregated with current payload count {}", count);
            }
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
                                Submit::EthereumContract(submission) => {
                                    if let Err(e) = _self.submit_to_ethereum(submission, packet).await {
                                        tracing::error!("{:?}", e);
                                    }
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
            .eth_mnemonic_hd_index_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let config = self
            .chain_configs
            .get(service.manager.chain_name())
            .ok_or(SubmissionError::MissingEthereumChain)?;

        let config: EthereumChainConfig = config
            .clone()
            .try_into()
            .map_err(|_| SubmissionError::MissingEthereumChain)?;

        let client = EthClientBuilder::new(config.to_client_config(
            Some(hd_index),
            Some(self.eth_mnemonic.clone()),
            Some(EthClientTransport::Http),
        ))
        .build_signing()
        .await
        .map_err(SubmissionError::Ethereum)?;

        tracing::info!(
            "Created new eth client for service {} -> {}",
            service.id,
            client.address()
        );

        self.eth_signing_clients
            .write()
            .unwrap()
            .insert(service.id.clone(), client);

        for workflow in service.workflows.values() {
            if let Submit::EthereumContract(EthereumContractSubmission { chain_name, .. }) =
                &workflow.submit
            {
                if !self
                    .eth_sending_clients
                    .read()
                    .unwrap()
                    .contains_key(chain_name)
                {
                    let config = self
                        .chain_configs
                        .get(chain_name)
                        .ok_or(SubmissionError::MissingEthereumChain)?;

                    let config: EthereumChainConfig = config
                        .clone()
                        .try_into()
                        .map_err(|_| SubmissionError::MissingEthereumChain)?;

                    let client = EthClientBuilder::new(config.to_client_config(
                        None,
                        Some(self.eth_mnemonic.clone()),
                        Some(EthClientTransport::Http),
                    ))
                    .build_signing()
                    .await
                    .map_err(SubmissionError::Ethereum)?;

                    self.eth_sending_clients
                        .write()
                        .unwrap()
                        .insert(chain_name.clone(), client);
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
        self.eth_signing_clients
            .read()
            .unwrap()
            .get(&service_id)
            .ok_or(SubmissionError::MissingMnemonic)
            .map(|client| {
                let key_bytes = client.signer.credential().to_bytes().to_vec();

                SigningKeyResponse::Secp256k1(key_bytes)
            })
    }
}
