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
    bindings::hello_world::temp_deserialize_hello_world_component_response,
    config::{Config, CosmosChainConfig, EthereumChainConfig},
    AppContext,
};
use alloy::{primitives::Log, sol_types::SolEvent};
use alloy_rlp::{Decodable, RlpDecodable};
use lavs_apis::{id::TaskId, verifier_simple::ExecuteMsg as VerifierExecuteMsg};
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    eth_client::{EthClientBuilder, EthClientConfig, EthSigningClient},
    hello_world::{
        solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
        HelloWorldSimpleClient,
    },
};

#[derive(Clone)]
pub struct CoreSubmission {
    cosmos_clients: Arc<Mutex<HashMap<u32, SigningClient>>>,
    cosmos_chain: Option<ChainCosmosSubmission>,
    eth_clients: Arc<Mutex<HashMap<u32, EthSigningClient>>>,
    eth_chain: Option<ChainEthSubmission>,
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

        let eth_chain = config
            .try_ethereum_chain_config()
            .map_err(SubmissionError::Ethereum)?
            .map(ChainEthSubmission::new)
            .transpose()?;

        Ok(Self {
            cosmos_clients: Arc::new(Mutex::new(HashMap::new())),
            cosmos_chain,
            eth_clients: Arc::new(Mutex::new(HashMap::new())),
            eth_chain,
            http_client: reqwest::Client::new(),
        })
    }

    fn get_cosmos_chain(&self) -> Result<&ChainCosmosSubmission, SubmissionError> {
        self.cosmos_chain
            .as_ref()
            .ok_or(SubmissionError::MissingCosmosChain)
    }

    fn get_eth_chain(&self) -> Result<&ChainEthSubmission, SubmissionError> {
        self.eth_chain
            .as_ref()
            .ok_or(SubmissionError::MissingEthereumChain)
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
    async fn get_eth_client(&self, hd_index: u32) -> Result<EthSigningClient, SubmissionError> {
        {
            let lock = self.eth_clients.lock().unwrap();

            if let Some(client) = lock.get(&hd_index) {
                return Ok(client.clone());
            }
        }

        let mut client_config = self.get_eth_chain()?.client_config.clone();

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
        let _faucet_url = match self.get_eth_chain()?.faucet_url.clone() {
            Some(url) => url,
            None => {
                tracing::debug!("No faucet configured, skipping");
                return Ok(());
            }
        };

        todo!()
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Submission"))]
    async fn add_task_to_aggregator(
        &self,
        client: &EthSigningClient,
        task: &AddTaskRequest,
    ) -> Result<AddTaskResponse, SubmissionError> {
        let aggregator_msg_url = self
            .get_eth_chain()?
            .aggregator_url
            .as_ref()
            .ok_or(SubmissionError::MissingAggregatorEndpoint)?
            .join("/msg")
            .unwrap();
        let response = self
            .http_client
            .post(aggregator_msg_url)
            .header("Content-Type", "application/json")
            .json(task)
            .send()
            .await
            .map_err(SubmissionError::Reqwest)?;
        let response: AddTaskResponse = response.json().await.map_err(SubmissionError::Reqwest)?;
        Ok(response)
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
                            let eth_client = match msg.submit {
                                Submit::EthSignedMessage{hd_index} => {
                                    let client = match _self.get_eth_client(hd_index).await {
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
                                Submit::EthAggregatorTx{} => {
                                    let hd_index = 0;
                                    let client = match _self.get_eth_client(hd_index).await {
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

                            let layer_client = match msg.submit {
                                Submit::EthSignedMessage{..} => {
                                    None
                                },
                                Submit::EthAggregatorTx{} => {
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
                                Submit::EthSignedMessage{.. } => {
                                    match msg.trigger_config.trigger {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address } => {
                                            let eth_client = eth_client.unwrap();

                                            let contract_address = match contract_address {
                                                Address::Eth(addr) => {
                                                    addr.as_bytes().into()
                                                },
                                                Address::Cosmos { .. } => {
                                                    tracing::error!("Expected Ethereum address, got cosmos {:?}", contract_address );
                                                    continue;
                                                }
                                            };

                                            #[derive(RlpDecodable)]
                                            pub struct EthOutput {
                                                pub address: Vec<u8>,
                                                pub log_topics: Vec<Vec<u8>>,
                                                pub log_data: Vec<u8>,
                                            }

                                            let EthOutput { address, log_topics, log_data } = match EthOutput::decode(&mut msg.wasm_result.as_slice()) {
                                                Ok(output) => output,
                                                Err(e) => {
                                                    tracing::error!("Failed to parse wasm result into rlp event output: {:?}", e);
                                                    continue;
                                                },
                                            };

                                            let address = alloy::primitives::Address::from_slice(address.as_slice());
                                            let log_topics = log_topics.into_iter().map(|t| alloy::primitives::FixedBytes::<32>::from_slice(t.as_slice())).collect();
                                            let log = match Log::new(address, log_topics, log_data.into()) {
                                                Some(log) => log,
                                                None => {
                                                    tracing::error!("Failed to create log from rlp event output");
                                                    continue;
                                                }
                                            };

                                            // This part is all specific to hello-world submission
                                            // TODO: Make this more generic
                                            let hello_world_event = match NewTaskCreated::decode_log(&log, false) {
                                                Ok(log) => log.data,
                                                Err(e) => {
                                                    tracing::error!("Failed to parse log data into NewTaskCreated event: {:?}", e);
                                                    continue;
                                                }
                                            };

                                            let avs_client = HelloWorldSimpleClient::new(eth_client, contract_address);

                                            let task_index = hello_world_event.taskIndex;
                                            match avs_client.sign_and_submit_task(hello_world_event.task, task_index).await {
                                                Ok(_) => {
                                                    tracing::debug!("Submission to Eth addr {} for task {} successful", avs_client.contract_address, response.task_index);
                                                },
                                                Err(e) => {
                                                    tracing::error!("Submission failed: {:?}", e);
                                                }
                                            }
                                        },
                                    }
                                },
                                Submit::EthAggregatorTx{} => {
                                    match msg.trigger_config.trigger  {
                                        Trigger::LayerQueue { .. } => {
                                            tracing::error!("Cross chain from Layer trigger to Ethereum submission is not supported yet");
                                            continue;
                                        },
                                        Trigger::EthEvent { contract_address } => {
                                            let eth_client = eth_client.unwrap();

                                            let contract_address = match contract_address {
                                                Address::Eth(addr) => {
                                                    addr.as_bytes().into()
                                                },
                                                Address::Cosmos { .. } => {
                                                    tracing::error!("Expected Ethereum address, got cosmos {:?}", contract_address);
                                                    continue;
                                                }
                                            };

                                            let avs_client = HelloWorldSimpleClient::new(eth_client.clone(), contract_address);

                                            let response = temp_deserialize_hello_world_component_response(&msg.wasm_result);

                                            let task = HelloWorldTask {
                                                name: response.task_name,
                                                taskCreatedBlock: response.task_created_block,
                                            };

                                            let task_index = response.task_index;

                                            // Generate request if possible
                                            let request = match avs_client.task_request(task, task_index).await {
                                                Ok(request) => {
                                                    request
                                                },
                                                Err(e) => {
                                                    tracing::error!("Submission failed: {:?}", e);
                                                    continue;
                                                }
                                            };
                                            match _self.add_task_to_aggregator(&eth_client, &request).await {
                                                Ok(response) => {
                                                    tracing::debug!("Aggregation to Eth addr {} for task {} successful", avs_client.contract_address, task_index);
                                                    if let Some(hash) = response.hash {
                                                        tracing::debug!("Task hash: {}", hash);
                                                    }
                                                },
                                                Err(e) => {
                                                    tracing::error!("Aggregation failed: {:?}", e);
                                                }
                                            }
                                        },
                                    }
                                },
                                Submit::LayerVerifierTx { verifier_addr, ..} => {
                                    match msg.trigger_config.trigger {
                                        Trigger::LayerQueue { task_queue_addr, .. } => {

                                            let result:serde_json::Value = match serde_json::from_slice(&msg.wasm_result) {
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

                                            match layer_client.unwrap().contract_execute(&verifier_addr, &contract_msg, Vec::new(), None).await {
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
