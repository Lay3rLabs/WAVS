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
    sol_types::SolValue,
};
use anyhow::anyhow;
use layer_climb::prelude::*;
use layer_cosmwasm::msg::LayerExecuteMsg;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    config::{CosmosChainConfig, EthereumChainConfig},
    eth_client::{EthChainConfig, EthClientBuilder, EthClientConfig, EthSigningClient},
    layer_contract_client::{layer_service_manager::LayerServiceManager, SignedPayload},
};

#[derive(Clone)]
pub struct CoreSubmission {
    cosmos_clients: Arc<Mutex<HashMap<u32, SigningClient>>>,
    cosmos_chain: Option<ChainCosmosSubmission>,
    eth_clients: Arc<Mutex<HashMap<(String, u32), EthSigningClient>>>,
    eth_chains: HashMap<String, ChainEthSubmission>,
    http_client: reqwest::Client,
}

#[derive(Clone)]
struct ChainCosmosSubmission {
    chain_config: ChainConfig,
    mnemonic: String,
    faucet_url: Option<Url>,
}

impl ChainCosmosSubmission {
    #[instrument(level = "debug", fields(subsys = "Submission"))]
    fn new(config: CosmosChainConfig, mnemonic: String) -> Result<Self, SubmissionError> {
        let faucet_url = config
            .faucet_endpoint
            .as_ref()
            .map(|url| Url::parse(url).map_err(SubmissionError::FaucetUrl))
            .transpose()?;

        Ok(Self {
            chain_config: config.into(),
            mnemonic,
            faucet_url,
        })
    }
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
        let cosmos_chain = config
            .try_cosmos_chain_config()
            .map_err(SubmissionError::Climb)?
            .map(|x| {
                let mnemonic = config
                    .cosmos_submission_mnemonic
                    .clone()
                    .ok_or(SubmissionError::MissingMnemonic)?;

                ChainCosmosSubmission::new(x.clone(), mnemonic)
            })
            .transpose()?;

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
            cosmos_clients: Arc::new(Mutex::new(HashMap::new())),
            cosmos_chain,
            eth_clients: Arc::new(Mutex::new(HashMap::new())),
            eth_chains,
            http_client: reqwest::Client::new(),
        })
    }

    fn get_cosmos_chain(&self) -> Result<&ChainCosmosSubmission, SubmissionError> {
        self.cosmos_chain
            .as_ref()
            .ok_or(SubmissionError::MissingCosmosChain)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn get_cosmos_client(
        &self,
        chain_name: String,
    ) -> Result<SigningClient, SubmissionError> {
        // TODO - where should hd_index come from?
        let hd_index = 0;

        {
            let lock = self.cosmos_clients.lock().unwrap();

            if let Some(client) = lock.get(&hd_index) {
                return Ok(client.clone());
            }
        }

        let derivation = cosmos_hub_derivation(hd_index).map_err(SubmissionError::Climb)?;

        let signer =
            KeySigner::new_mnemonic_str(&self.get_cosmos_chain()?.mnemonic, Some(&derivation))
                .map_err(SubmissionError::Climb)?;

        let client =
            SigningClient::new(self.get_cosmos_chain()?.chain_config.clone(), signer, None)
                .await
                .map_err(SubmissionError::Climb)?;

        {
            let mut lock = self.cosmos_clients.lock().unwrap();
            lock.insert(hd_index, client.clone());
        }

        Ok(client)
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
    async fn maybe_tap_cosmos_faucet(&self, client: &SigningClient) -> Result<(), SubmissionError> {
        let faucet_url = match self.get_cosmos_chain()?.faucet_url.clone() {
            Some(url) => url,
            None => {
                tracing::debug!("No faucet configured, skipping");
                return Ok(());
            }
        };

        let balance = client
            .querier
            .balance(client.addr.clone(), None)
            .await
            .map_err(SubmissionError::Climb)?
            .unwrap_or_default();

        tracing::debug!("Client {} has balance: {}", client.addr, balance);

        let required_funds =
            (10_000_000f32 * self.get_cosmos_chain()?.chain_config.gas_price).round() as u128;

        if balance > required_funds {
            return Ok(());
        }

        let body = serde_json::json!({
            "address": client.addr.to_string(),
            "denom": self.get_cosmos_chain()?.chain_config.gas_denom.clone()
        })
        .to_string();

        tracing::debug!("Tapping faucet at {} with {}", faucet_url, body);

        let res = self
            .http_client
            .post(faucet_url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(SubmissionError::Reqwest)?;

        if !res.status().is_success() {
            let body = res.text().await.map_err(SubmissionError::Reqwest)?;
            return Err(SubmissionError::Faucet(body));
        }

        if cfg!(debug_assertions) {
            let balance = client
                .querier
                .balance(client.addr.clone(), None)
                .await
                .map_err(SubmissionError::Climb)?
                .unwrap_or_default();
            tracing::debug!("After faucet tap, {} has balance: {}", client.addr, balance);
        }

        Ok(())
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

        let data_hash = eip191_hash_message(keccak256(data.abi_encode()));
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
                .addSignedPayload(
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

    async fn submit_to_cosmos(
        &self,
        _chain_name: String,
        cosmos_client: SigningClient,
        contract_addr: Address,
        data: Vec<u8>,
    ) -> Result<(), SubmissionError> {
        // TODO - commitments etc.
        let signature = Vec::new();
        let contract_msg = LayerExecuteMsg::new(data, signature);

        let _tx_resp = cosmos_client
            .contract_execute(&contract_addr, &contract_msg, Vec::new(), None)
            .await
            .map_err(SubmissionError::FailedToSubmitCosmos)?;

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
                                Submit::CosmosContract { chain_name, contract_addr } => {
                                    let client = match _self.get_cosmos_client(chain_name.to_string()).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_cosmos_faucet(&client).await {
                                        tracing::error!("Failed to tap faucet for client {}: {:?}",client.addr, err);
                                    }

                                    if let Err(e) = _self.submit_to_cosmos(chain_name.to_string(), client, contract_addr, msg.wasm_result).await {
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
