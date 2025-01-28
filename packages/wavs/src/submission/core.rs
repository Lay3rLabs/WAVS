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
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    avs_client::{ServiceManagerClient, SignedPayload},
    config::{AnyChainConfig, EthereumChainConfig},
    eth_client::{EthClientBuilder, EthClientTransport, EthSigningClient},
    types::{ChainName, Submit},
};

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

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    #[allow(clippy::too_many_arguments)]
    async fn submit_to_ethereum(
        &self,
        chain_name: ChainName,
        service_manager_address: alloy::primitives::Address,
        data: Vec<u8>,
        max_gas: Option<u64>,
    ) -> Result<(), SubmissionError> {
        let eth_client = self
            .get_eth_client(&chain_name)
            .await
            .map_err(|_| SubmissionError::MissingEthereumChain)?;

        let data_hash = eip191_hash_message(keccak256(&data));
        let signature: Vec<u8> = eth_client
            .signer
            .sign_hash_sync(&data_hash)
            .map_err(|_| SubmissionError::FailedToSignPayload)?
            .into();

        let signed_payload = SignedPayload {
            operator: eth_client.address(),
            data,
            data_hash,
            signature,
            signed_block_height: eth_client
                .provider
                .get_block_number()
                .await
                .map_err(|e| SubmissionError::Ethereum(anyhow!("{}", e)))?
                - 1,
        };

        if let Some(aggregator_url) = self
            .chain_configs
            .get(&chain_name)
            .and_then(|chain| EthereumChainConfig::try_from(chain.clone()).ok())
            .and_then(|config| config.aggregator_endpoint.clone())
        {
            let response = self
                .http_client
                .post(format!("{aggregator_url}/add-payload"))
                .header("Content-Type", "application/json")
                .json(&AggregateAvsRequest::EigenContract {
                    signed_payload,
                    service_manager_address,
                })
                .send()
                .await
                .map_err(SubmissionError::Reqwest)?;

            if !response.status().is_success() {
                return Err(SubmissionError::Aggregator(format!(
                    "error hitting {aggregator_url} response: {:?}",
                    response
                )));
            }

            let response: AggregateAvsResponse =
                response.json().await.map_err(SubmissionError::Reqwest)?;

            match response {
                AggregateAvsResponse::Sent { tx_hash, count } => {
                    tracing::debug!(
                        "Aggregator submitted with tx hash {} and payload count {}",
                        tx_hash,
                        count
                    );
                }
                AggregateAvsResponse::Aggregated { count } => {
                    tracing::debug!("Aggregated with current payload count {}", count);
                }
            }
        } else {
            if let Err(err) = self.maybe_tap_eth_faucet(chain_name, &eth_client).await {
                tracing::error!(
                    "Failed to tap faucet for client {}: {:?}",
                    eth_client.address(),
                    err
                );
            }

            ServiceManagerClient::new(eth_client.clone(), service_manager_address)
                .add_signed_payload(signed_payload, max_gas)
                .await
                .map_err(|e| SubmissionError::Ethereum(anyhow!("{}", e)))?;
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
                            match msg.submit {
                                Submit::EigenContract {chain_name, service_manager, max_gas } => {
                                    if let Err(e) = _self.submit_to_ethereum(chain_name, service_manager, msg.wasi_result, max_gas).await {
                                        tracing::error!("{:?}", e);
                                    }
                                },
                                Submit::None => {
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
