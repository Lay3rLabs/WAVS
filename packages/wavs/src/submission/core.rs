use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    apis::{
        dispatcher::{Submit, SubmitFormat},
        submission::{ChainMessage, Submission, SubmissionError, SubmitWrapper},
        trigger::{Trigger, TriggerData},
    },
    config::Config,
    AppContext,
};
use anyhow::anyhow;
use lavs_apis::{id::TaskId, verifier_simple::ExecuteMsg as VerifierExecuteMsg};
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    config::{CosmosChainConfig, EthereumChainConfig},
    eth_client::{EthChainConfig, EthClientBuilder, EthClientConfig, EthSigningClient},
    layer_contract_client::LayerContractClientSimple,
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
    async fn get_cosmos_client(&self, hd_index: u32) -> Result<SigningClient, SubmissionError> {
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
        hd_index: u32,
    ) -> Result<EthSigningClient, SubmissionError> {
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
    async fn submit_to_ethereum(
        &self,
        chain_name: String,
        eth_client: EthSigningClient,
        trigger_address: Address,
        service_manager_address: Address,
        data: Vec<u8>,
        aggregate: bool,
    ) -> Result<(), SubmissionError> {
        let trigger_address = match trigger_address {
            Address::Eth(addr) => addr.as_bytes().into(),
            _ => {
                return Err(SubmissionError::ExpectedEthAddress(
                    trigger_address.to_string(),
                ))
            }
        };

        let service_manager_address = match service_manager_address {
            Address::Eth(addr) => addr.as_bytes().into(),
            _ => {
                return Err(SubmissionError::ExpectedEthAddress(
                    service_manager_address.to_string(),
                ))
            }
        };

        let avs_client =
            LayerContractClientSimple::new(eth_client, service_manager_address, trigger_address);

        let signed_payload = avs_client
            .sign_payload(data)
            .await
            .map_err(|_| SubmissionError::FailedToSignPayload)?;

        if aggregate {
            let request = AggregateAvsRequest::EthTrigger {
                signed_payload,
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
            match avs_client.add_signed_payload(signed_payload).await {
                Ok(_) => {
                    tracing::debug!("Submission to Eth successful!");
                }
                Err(e) => {
                    return Err(SubmissionError::FailedToSubmitEthDirect(e));
                }
            }
        }

        Ok(())
    }

    async fn submit_to_cosmos(
        &self,
        cosmos_client: SigningClient,
        verifier_addr: Address,
        task_queue_addr: Address,
        data: Vec<u8>,
    ) -> Result<(), SubmissionError> {
        // TODO - formalize this, used to be VerifierExecuteMsg::ExecutedTask
        let contract_msg = serde_json::json!({
            "task_queue_contract": task_queue_addr.to_string(),
            // "task_id": task_id, - could be derived from trigger action data... but.. not necessary...
            "result": data,
        });

        // this will currently break, need to migrate verifier addr format
        let _tx_resp = cosmos_client
            .contract_execute(&verifier_addr, &contract_msg, Vec::new(), None)
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

                            let submit_data = match msg.submit_format {
                                SubmitFormat::InputOutputId => {
                                    match msg.trigger.data {
                                        TriggerData::RawWithId { data: input, id } => {
                                            serde_json::to_vec(&SubmitWrapper {
                                                input: Some(input),
                                                id: Some(id),
                                                data: msg.wasm_result.clone(),
                                            }).map_err(SubmissionError::Serde)
                                        },
                                        _ => {
                                            Err(SubmissionError::MismatchTriggerFormat)
                                        }
                                    }
                                },
                                SubmitFormat::OutputId => {
                                    match msg.trigger.data {
                                        TriggerData::RawWithId { id, .. } => {
                                            serde_json::to_vec(&SubmitWrapper {
                                                input: None, 
                                                id: Some(id),
                                                data: msg.wasm_result.clone(),
                                            }).map_err(SubmissionError::Serde)
                                        },
                                        _ => {
                                            Err(SubmissionError::MismatchTriggerFormat)
                                        }
                                    }
                                },
                                SubmitFormat::Raw => {
                                    Ok(msg.wasm_result.clone())
                                },
                            };

                            let submit_data = match submit_data {
                                Ok(data) => data,
                                Err(e) => {
                                    tracing::error!("Failed to serialize submit data: {:?}", e);
                                    continue;
                                }
                            };


                            let eth_client = match msg.submit {
                                Submit::EthSignedMessage{hd_index, chain_name, .. } => {
                                    let client = match _self.get_eth_client(chain_name.to_string(), hd_index).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_eth_faucet(chain_name.to_string(), &client).await {
                                        tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.address(), hd_index, err);
                                    }

                                    Some(client)
                                },
                                Submit::EthAggregatorTx{chain_name, ..} => {
                                    let hd_index = 0;
                                    let client = match _self.get_eth_client(chain_name.to_string(), hd_index).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_eth_faucet(chain_name.to_string(), &client).await {
                                        tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.address(), hd_index, err);
                                    }

                                    Some(client)
                                },
                                Submit::LayerVerifierTx { .. } => {
                                    None
                                }
                            };

                            let layer_client = match msg.submit {
                                Submit::EthSignedMessage{..} => {
                                    None
                                },
                                Submit::EthAggregatorTx{..} => {
                                    None
                                },
                                Submit::LayerVerifierTx { hd_index, .. } => {
                                    let client = match _self.get_cosmos_client(hd_index).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_cosmos_faucet(&client).await {
                                        tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.addr, hd_index, err);
                                    }

                                    Some(client)
                                }
                            };

                            match msg.submit {
                                Submit::EthSignedMessage{service_manager_addr, chain_name, ..} => {
                                    match &msg.trigger.config.trigger {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address: trigger_addr } => {
                                            if let Err(e) = _self.submit_to_ethereum(chain_name.to_string(), eth_client.unwrap(), trigger_addr.clone(), service_manager_addr.clone(), submit_data, false).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        },
                                    }
                                },
                                Submit::EthAggregatorTx{service_manager_addr, chain_name} => {
                                    match &msg.trigger.config.trigger {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address: trigger_addr } => {
                                            if let Err(e) = _self.submit_to_ethereum(chain_name.to_string(), eth_client.unwrap(), trigger_addr.clone(), service_manager_addr.clone(), submit_data, true).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        },
                                    }
                                },
                                Submit::LayerVerifierTx { verifier_addr, ..} => {
                                    match &msg.trigger.config.trigger {
                                        Trigger::LayerQueue { task_queue_addr, .. } => {

                                            if let Err(e) = _self.submit_to_cosmos(layer_client.unwrap(), verifier_addr.clone(), task_queue_addr.clone(), msg).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        }

                                        Trigger::EthEvent { .. } => {
                                            tracing::error!("Cross chain from Ethereum trigger to Layer submission is not supported yet");
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        tracing::debug!("Submission channel closed");
                    }
                }
            }
        });

        Ok(())
    }
}
