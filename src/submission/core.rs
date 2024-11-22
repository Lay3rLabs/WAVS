use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    apis::{
        submission::{ChainMessage, Submission, SubmissionError},
        Trigger,
    },
    config::Config,
    context::AppContext,
};
use lavs_apis::verifier_simple::ExecuteMsg as VerifierExecuteMsg;
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;

#[derive(Clone)]
pub struct CoreSubmission {
    clients: Arc<Mutex<HashMap<u32, SigningClient>>>,
    chain_config: ChainConfig,
    mnemonic: String,
    faucet_url: Option<Url>,
    http_client: reqwest::Client,
}

impl CoreSubmission {
    #[allow(clippy::new_without_default)]
    #[instrument(fields(subsys = "Submission"))]
    pub fn new(config: &Config) -> Result<Self, SubmissionError> {
        let wasmatic_chain_config = config
            .wasmatic_chain_config()
            .map_err(SubmissionError::Climb)?;

        Ok(Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            mnemonic: wasmatic_chain_config
                .submission_mnemonic
                .clone()
                .ok_or(SubmissionError::MissingMnemonic)?,
            faucet_url: wasmatic_chain_config
                .faucet_endpoint
                .as_ref()
                .map(|url| Url::parse(url).map_err(SubmissionError::FaucetUrl))
                .transpose()?,
            chain_config: wasmatic_chain_config.into(),
            http_client: reqwest::Client::new(),
        })
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    async fn get_client(&self, hd_index: u32) -> Result<SigningClient, SubmissionError> {
        {
            let lock = self.clients.lock().unwrap();

            if let Some(client) = lock.get(&hd_index) {
                return Ok(client.clone());
            }
        }

        let derivation = cosmos_hub_derivation(hd_index).map_err(SubmissionError::Climb)?;

        let signer = KeySigner::new_mnemonic_str(&self.mnemonic, Some(&derivation))
            .map_err(SubmissionError::Climb)?;

        let client = SigningClient::new(self.chain_config.clone(), signer)
            .await
            .map_err(SubmissionError::Climb)?;

        {
            let mut lock = self.clients.lock().unwrap();
            lock.insert(hd_index, client.clone());
        }

        Ok(client)
    }

    #[instrument(skip(self), fields(subsys = "Submission"))]
    async fn maybe_tap_faucet(&self, client: &SigningClient) -> Result<(), SubmissionError> {
        let faucet_url = match self.faucet_url.clone() {
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

        let required_funds = (10_000_000f32 * self.chain_config.gas_price).round() as u128;

        if balance > required_funds {
            return Ok(());
        }

        let body = serde_json::json!({
            "address": client.addr.to_string(),
            "denom": self.chain_config.gas_denom.clone()
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
}

impl Submission for CoreSubmission {
    #[instrument(skip(self, ctx), fields(subsys = "Submission"))]
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

                            let client = match _self.get_client(msg.hd_index).await {
                                Ok(client) => client,
                                Err(e) => {
                                    tracing::error!("Failed to get client: {:?}", e);
                                    continue;
                                }
                            };

                            if let Err(err) = _self.maybe_tap_faucet(&client).await {
                                tracing::error!("Failed to tap faucet for client {} at hd_index {}: {:?}",client.addr, msg.hd_index, err);
                            }

                            let result:serde_json::Value = match serde_json::from_slice(&msg.wasm_result) {
                                Ok(result) => result,
                                Err(e) => {
                                    tracing::error!("Failed to parse wasm result into json value: {:?}", e);
                                    continue;
                                }
                            };

                            let result = match serde_json::to_string(&result) {
                                Ok(result) => result,
                                Err(e) => {
                                    tracing::error!("Failed to serialize json value into string: {:?}", e);
                                    continue;
                                }
                            };

                            let contract_msg = match msg.trigger_data.trigger {
                                Trigger::Queue { task_queue_addr, .. } => {
                                    VerifierExecuteMsg::ExecutedTask {
                                        task_queue_contract: task_queue_addr.to_string(),
                                        task_id: msg.task_id,
                                        result,
                                    }
                                }
                            };

                            match client.contract_execute(&msg.verifier_addr, &contract_msg, Vec::new(), None).await {
                                Ok(_) => {
                                    tracing::debug!("Submission successful");
                                },
                                Err(e) => {
                                    tracing::error!("Submission failed: {:?}", e);
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
