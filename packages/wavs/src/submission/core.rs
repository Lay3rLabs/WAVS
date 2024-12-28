use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    apis::{
        dispatcher::Submit,
        submission::{ChainMessage, Submission, SubmissionError},
        trigger::Trigger,
    },
    config::{Config, CosmosChainConfig, EthereumChainConfig},
    AppContext,
};
use alloy::primitives::Address as EthAddress;
use lavs_apis::{id::TaskId, verifier_simple::ExecuteMsg as VerifierExecuteMsg};
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    eth_client::{EthClientBuilder, EthClientConfig, EthSigningClient},
    layer_contract_client::LayerContractClientSimple,
};

#[derive(Clone)]
pub struct CoreSubmission {
    cosmos_clients: Arc<Mutex<HashMap<u32, SigningClient>>>,
    cosmos_chain: Option<ChainCosmosSubmission>,
    eth_clients: Arc<Mutex<HashMap<u32, EthSigningClient>>>,
    // chain_id -> chain
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
    fn new(config: CosmosChainConfig) -> Result<Self, SubmissionError> {
        let mnemonic = config
            .submission_mnemonic
            .clone()
            .ok_or(SubmissionError::MissingMnemonic)?;

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
    fn new(config: EthereumChainConfig) -> Result<Self, SubmissionError> {
        let aggregator_url = config
            .aggregator_endpoint
            .as_ref()
            .map(|endpoint| endpoint.parse())
            .transpose()
            .map_err(SubmissionError::AggregatorUrl)?;
        let client_config = config.into();

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
            .map(ChainCosmosSubmission::new)
            .transpose()?;

        let eth_chains = config
            .ethereum_chain_configs()
            .into_iter()
            .flat_map(|hm| hm.into_iter())
            .map(|(chain_id, ecc)| Ok((chain_id, ChainEthSubmission::new(ecc)?)))
            .collect::<Result<HashMap<_, _>, SubmissionError>>()?;

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

    fn get_eth_chain(
        &self,
        chain_id: impl AsRef<str>,
    ) -> Result<&ChainEthSubmission, SubmissionError> {
        let chain =
            self.eth_chains
                .get(chain_id.as_ref())
                .ok_or(SubmissionError::MissingEthereumChain(
                    chain_id.as_ref().to_string(),
                ))?;
        Ok(chain)
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

        let client = SigningClient::new(self.get_cosmos_chain()?.chain_config.clone(), signer)
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
        chain_id: &str,
        hd_index: u32,
    ) -> Result<EthSigningClient, SubmissionError> {
        {
            let lock = self.eth_clients.lock().unwrap();

            if let Some(client) = lock.get(&hd_index) {
                return Ok(client.clone());
            }
        }

        let mut client_config = self.get_eth_chain(chain_id)?.client_config.clone();

        client_config.hd_index = Some(hd_index);

        let client = EthClientBuilder::new(client_config)
            .build_signing()
            .await
            .map_err(SubmissionError::Ethereum)?;

        {
            let mut lock = self.eth_clients.lock().unwrap();
            lock.insert(hd_index, client.clone());
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
    async fn maybe_tap_eth_faucet(&self, client: &EthSigningClient) -> Result<(), SubmissionError> {
        let _faucet_url = match self
            .get_eth_chain(&client.config.chain_id)?
            .faucet_url
            .clone()
        {
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
        eth_client: EthSigningClient,
        trigger_address: Address,
        service_manager_address: Address,
        msg: ChainMessage,
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

        let service_manager_address: EthAddress = match service_manager_address {
            Address::Eth(addr) => addr.as_bytes().into(),
            _ => {
                return Err(SubmissionError::ExpectedEthAddress(
                    service_manager_address.to_string(),
                ))
            }
        };

        let chain_id = eth_client.config.chain_id.to_string();
        let avs_client =
            LayerContractClientSimple::new(eth_client, service_manager_address, trigger_address);

        let (trigger_id, wasm_result, service_id) = match msg {
            ChainMessage::Eth {
                trigger_id,
                wasm_result,
                trigger_config,
                ..
            } => (trigger_id, wasm_result, trigger_config.service_id),
            _ => {
                return Err(SubmissionError::ExpectedEthMessage);
            }
        };

        let signed_payload = avs_client
            .sign_payload(trigger_id, wasm_result)
            .await
            .map_err(|_| SubmissionError::FailedToSignPayload)?;

        if aggregate {
            let request = AggregateAvsRequest::EthTrigger {
                signed_payload,
                service_manager_address,
                chain_name: "e2elocal2".to_string(), // TODO: reece convert from chain_id -> chain_name somehow? (or use chain_id)
                service_id: service_id.to_string(),
            };

            let aggregator_msg_url = self
                .get_eth_chain(chain_id)?
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
                    tracing::debug!(
                        "Submission to Eth for trigger id {} successful!",
                        trigger_id
                    );
                }
                Err(e) => {
                    return Err(SubmissionError::FailedToSubmitEthDirect(e));
                }
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
                            tracing::debug!("Received message to submit: {:?}", msg);
                            let eth_client = match msg.submit() {
                                // TODO(reece): just use the hd_index to get the chain_id pairings? (better UX)
                                Submit::EthSignedMessage{chain_id, hd_index, .. } => {

                                    let client = match _self.get_eth_client(chain_id.as_str(), *hd_index).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_eth_faucet(&client).await {
                                        tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.address(), hd_index, err);
                                    }

                                    Some(client)
                                },
                                Submit::EthAggregatorTx{chain_id, ..} => {
                                    let hd_index = 0;
                                    let client = match _self.get_eth_client(chain_id.as_str(), hd_index).await {
                                        Ok(client) => client,
                                        Err(e) => {
                                            tracing::error!("Failed to get client: {:?}", e);
                                            continue;
                                        }
                                    };

                                    if let Err(err) = _self.maybe_tap_eth_faucet(&client).await {
                                        tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.address(), hd_index, err);
                                    }

                                    Some(client)
                                },
                                Submit::LayerVerifierTx { .. } => {
                                    None
                                }
                            };

                            let layer_client = match msg.submit() {
                                Submit::EthSignedMessage{..} => {
                                    None
                                },
                                Submit::EthAggregatorTx{..} => {
                                    None
                                },
                                Submit::LayerVerifierTx { hd_index, .. } => {
                                    let client = match _self.get_cosmos_client(*hd_index).await {
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

                            match msg.submit() {
                                Submit::EthSignedMessage{service_manager_addr, ..} => {
                                    match &msg.trigger_config().trigger {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address: trigger_addr } => {
                                            if let Err(e) = _self.submit_to_ethereum(eth_client.unwrap(), trigger_addr.clone(), service_manager_addr.clone(), msg, false).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        },
                                    }
                                },
                                Submit::EthAggregatorTx{service_manager_addr, ..} => {
                                    match &msg.trigger_config().trigger  {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address: trigger_addr } => {
                                            if let Err(e) = _self.submit_to_ethereum(eth_client.unwrap(), trigger_addr.clone(), service_manager_addr.clone(), msg, true).await {
                                                tracing::error!("{:?}", e);
                                            }
                                        },
                                    }
                                },
                                Submit::LayerVerifierTx { verifier_addr, ..} => {
                                    match &msg.trigger_config().trigger {
                                        Trigger::LayerQueue { task_queue_addr, .. } => {

                                            let result:serde_json::Value = match serde_json::from_slice(msg.wasm_result()) {
                                                Ok(result) => result,
                                                Err(e) => {
                                                    tracing::error!("Failed to parse wasm result into json value: {:?}", e);
                                                    continue;
                                                }
                                            };

                                            // TODO - TaskId is a TaskQueue concept, not all triggers will have it, should be part of result
                                            let task_id = TaskId::new(result.get("task_id").unwrap().as_u64().unwrap());

                                            let result = match serde_json::to_string(&result) {
                                                Ok(result) => result,
                                                Err(e) => {
                                                    tracing::error!("Failed to serialize json value into string: {:?}", e);
                                                    continue;
                                                }
                                            };


                                            let contract_msg = VerifierExecuteMsg::ExecutedTask {
                                                task_queue_contract: task_queue_addr.to_string(),
                                                task_id,
                                                result,
                                            };

                                            match layer_client.unwrap().contract_execute(verifier_addr, &contract_msg, Vec::new(), None).await {
                                                Ok(_) => {
                                                    tracing::debug!("Submission successful");
                                                },
                                                Err(e) => {
                                                    tracing::error!("Submission failed: {:?}", e);
                                                }
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
