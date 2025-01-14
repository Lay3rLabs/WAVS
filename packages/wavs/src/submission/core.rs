use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    apis::{
        dispatcher::Submit,
        submission::{ChainMessage, Submission, SubmissionError},
    },
    config::Config,
    AppContext,
};
use alloy::signers::SignerSync;
use alloy::{
    primitives::{eip191_hash_message, keccak256},
    providers::Provider,
};
use anyhow::anyhow;
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    avs_client::{layer_service_manager::LayerServiceManager, SignedPayload},
    config::EthereumChainConfig,
    eth_client::{EthChainConfig, EthClientBuilder, EthClientConfig, EthSigningClient},
};

#[derive(Clone)]
pub struct CoreSubmission {
    eth_clients: Arc<Mutex<HashMap<(String, u32), EthSigningClient>>>,
    eth_chains: HashMap<String, ChainEthSubmission>,
    http_client: reqwest::Client,
}

#[derive(Clone)]
struct ChainEthSubmission {
    client_config: EthClientConfig,
    aggregator_url: Option<Url>,
    faucet_url: Option<Url>,
}

impl ChainEthSubmission {
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    fn new(config: EthereumChainConfig, mnemonic: String) -> Result<Self, SubmissionError> {
        let aggregator_url = config
            .aggregator_endpoint
            .as_ref()
            .map(|endpoint| endpoint.parse())
            .transpose()
            .map_err(SubmissionError::AggregatorUrl)?;

        let client_config = EthChainConfig::from(config).to_client_config(None, Some(mnemonic));

        Ok(Self {
            client_config,
            aggregator_url,
            // TODO: Ethereum faucet
            faucet_url: None,
        })
    }
}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    pub fn new(config: &Config) -> Result<Self, SubmissionError> {
        let mut eth_chains = HashMap::new();
        let active_ethereum_chain_configs = config.active_ethereum_chain_configs();
        if !active_ethereum_chain_configs.is_empty() {
            let mnemonic = config
                .submission_mnemonic
                .clone()
                .ok_or(SubmissionError::MissingMnemonic)?;

            for (name, chain_config) in config.active_ethereum_chain_configs() {
                eth_chains.insert(
                    name.clone(),
                    ChainEthSubmission::new(chain_config, mnemonic.clone())?,
                );
            }
        }

        Ok(Self {
            eth_clients: Arc::new(Mutex::new(HashMap::new())),
            eth_chains,
            http_client: reqwest::Client::new(),
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn get_eth_client(
        &self,
        chain_name: String,
    ) -> Result<EthSigningClient, SubmissionError> {
        // TODO - where should hd_index come from?
        let hd_index = 0;

        {
            let lock = self.eth_clients.lock().unwrap();

            if let Some(client) = lock.get(&(chain_name.clone(), hd_index)) {
                return Ok(client.clone());
            }
        }

        let mut client_config = self
            .eth_chains
            .get(&chain_name)
            .ok_or(SubmissionError::MissingCosmosChain)?
            .client_config
            .clone();

        client_config.hd_index = Some(hd_index);

        let client = EthClientBuilder::new(client_config)
            .build_signing()
            .await
            .map_err(SubmissionError::Ethereum)?;

        {
            let mut lock = self.eth_clients.lock().unwrap();
            lock.insert((chain_name, hd_index), client.clone());
        }

        Ok(client)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn maybe_tap_eth_faucet(
        &self,
        chain_name: String,
        client: &EthSigningClient,
    ) -> Result<(), SubmissionError> {
        let chain_config = self
            .eth_chains
            .get(&chain_name)
            .ok_or(SubmissionError::MissingEthereumChain)?;
        let _faucet_url = match chain_config.faucet_url.clone() {
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
        chain_name: String,
        eth_client: EthSigningClient,
        service_manager_address: Address,
        data: Vec<u8>,
        aggregate: bool,
        max_gas: Option<u64>,
    ) -> Result<(), SubmissionError> {
        let service_manager_address = match service_manager_address {
            Address::Eth(addr) => addr.as_bytes().into(),
            _ => {
                return Err(SubmissionError::ExpectedEthAddress(
                    service_manager_address.to_string(),
                ))
            }
        };

        let service_manager_contract =
            LayerServiceManager::new(service_manager_address, eth_client.provider.clone());

        let data_hash = eip191_hash_message(keccak256(&data));
        let signature: Vec<u8> = eth_client
            .signer
            .sign_hash_sync(&data_hash)
            .map_err(|_| SubmissionError::FailedToSignPayload)?
            .into();

        if aggregate {
            let request = AggregateAvsRequest::EigenContract {
                signed_payload: SignedPayload {
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
                },
                service_manager_address,
            };

            let chain_config = self
                .eth_chains
                .get(&chain_name)
                .ok_or(SubmissionError::MissingEthereumChain)?;

            let aggregator_msg_url = chain_config
                .aggregator_url
                .as_ref()
                .ok_or(SubmissionError::MissingAggregatorEndpoint)?
                .join("/add-payload")
                .unwrap();

            let response = self
                .http_client
                .post(aggregator_msg_url)
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(SubmissionError::Reqwest)?;

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
            let _ = service_manager_contract
                .addPayload(
                    SignedPayload {
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
                    }
                    .into_submission_abi(),
                )
                .gas(max_gas.unwrap_or(500_000).min(30_000_000))
                .send()
                .await
                .map_err(|e| SubmissionError::FailedToSubmitEthDirect(anyhow!("{}", e)))?;
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
                                Submit::EigenContract {chain_name, aggregate, service_manager } => {
                                    if aggregate {
                                    } else {
                                        let client = match _self.get_eth_client(chain_name.to_string()).await {
                                            Ok(client) => client,
                                            Err(e) => {
                                                tracing::error!("Failed to get client: {:?}", e);
                                                continue;
                                            }
                                        };

                                        if let Err(err) = _self.maybe_tap_eth_faucet(chain_name.to_string(), &client).await {
                                            tracing::error!("Failed to tap faucet for client {}: {:?}",client.address(), err);
                                        }

                                        if let Err(e) = _self.submit_to_ethereum(chain_name.to_string(), client, service_manager, msg.wasm_result, aggregate).await {
                                            tracing::error!("{:?}", e);
                                        }
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
