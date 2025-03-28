use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use crate::{
    apis::submission::{ChainMessage, Submission, SubmissionError},
    config::Config,
    AppContext,
};
use alloy::signers::SignerSync;
use alloy::{
    primitives::{eip191_hash_message, keccak256},
    providers::Provider,
};
use anyhow::anyhow;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    config::{AnyChainConfig, EthereumChainConfig},
    eth_client::{EthClientBuilder, EthClientTransport, EthSigningClient},
};
use wavs_types::{aggregator::{AddPacketRequest, AddPacketResponse}, ChainName, Envelope, EthereumContractSubmission, Packet, PacketRoute, SignerAddress, Submit};

#[derive(Clone)]
pub struct CoreSubmission {
    chain_configs: BTreeMap<ChainName, AnyChainConfig>,
    http_client: reqwest::Client,
    // created on-demand from chain_name and hd_index
    eth_clients: Arc<Mutex<HashMap<(ChainName, u32), EthSigningClient>>>,
    eth_mnemonic: String,
}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    pub fn new(config: &Config) -> Result<Self, SubmissionError> {
        Ok(Self {
            chain_configs: config.chains.clone().into(),
            http_client: reqwest::Client::new(),
            eth_clients: Arc::new(Mutex::new(HashMap::new())),
            eth_mnemonic: config
                .submission_mnemonic
                .clone()
                .ok_or(SubmissionError::MissingMnemonic)?,
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn get_eth_client(
        &self,
        chain_name: &ChainName,
    ) -> Result<EthSigningClient, SubmissionError> {
        // TODO - where should hd_index come from?
        let hd_index = 0;

        if let Some(client) = self
            .eth_clients
            .lock()
            .unwrap()
            .get(&(chain_name.clone(), hd_index))
        {
            return Ok(client.clone());
        }

        let config = self
            .chain_configs
            .get(chain_name)
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

        self.eth_clients
            .lock()
            .unwrap()
            .insert((chain_name.clone(), hd_index), client.clone());

        Ok(client)
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

    async fn make_packet(&self, chain_name: ChainName, payload: Vec<u8>) -> Result<Packet, SubmissionError> {
        let eth_client = self
            .get_eth_client(&chain_name)
            .await
            .map_err(|_| SubmissionError::MissingEthereumChain)?;

        let block_height = eth_client
            .provider
            .get_block_number()
            .await
            .map_err(|e| SubmissionError::Ethereum(anyhow!("{}", e)))?
            - 1;

        let signer = eth_client.address();
        let envelope:Envelope = unimplemented!("envelope needs to be passed through");

        let signature = eth_client.sign_envelope(&envelope).await?;
        let route:PacketRoute = unimplemented!("route needs to be passed through");

        Ok(Packet { 
            route, 
            envelope, 
            signer: SignerAddress::Ethereum(signer),
            signature, 
            block_height
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
            address
        } = submission;



        let eth_client = self
            .get_eth_client(&chain_name)
            .await
            .map_err(|_| SubmissionError::MissingEthereumChain)?;

        if let Err(err) = self.maybe_tap_eth_faucet(chain_name, &eth_client).await {
            tracing::error!(
                "Failed to tap faucet for client {}: {:?}",
                eth_client.address(),
                err
            );
        }

        let signer_and_signatures = vec![(packet.signer, packet.signature)];

        let tx_receipt = eth_client.send_envelope_signatures(
            packet.envelope, 
            signer_and_signatures, 
            packet.block_height, 
            address, 
            max_gas
        ).await.map_err(|e| SubmissionError::FailedToSubmitEthDirect(e.into()))?;

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
            .json(&AddPacketRequest {
                packet
            })
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
                            let chain_name = match &msg.submit {
                                Submit::EthereumContract(submission) => {
                                    submission.chain_name.clone()
                                },
                                Submit::Aggregator{url} => {
                                    todo!()
                                    // TODO - get chain name from service.manager
                                }
                                Submit::None => {
                                    continue;
                                }
                            };

                            let packet = match _self.make_packet(chain_name, msg.wasi_result).await {
                                Ok(packet) => packet,
                                Err(e) => {
                                    tracing::error!("Failed to make packet: {:?}", e);
                                    continue;
                                }
                            };
                            
                            match msg.submit {
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
}
