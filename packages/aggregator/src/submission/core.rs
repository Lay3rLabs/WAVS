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
use alloy::rpc::types::TransactionRequest;
use lavs_apis::verifier_simple::ExecuteMsg as VerifierExecuteMsg;
use layer_climb::prelude::*;
use reqwest::Url;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::eth_client::EthSigningClient;

#[derive(Clone)]
pub struct CoreSubmission {
    signing_client: EthSigningClient,
    // clients: Arc<Mutex<HashMap<u32, SigningClient>>>,
    // chain_config: ChainConfig,
    // mnemonic: String,
    // faucet_url: Option<Url>,
    // http_client: reqwest::Client,
}

impl CoreSubmission {
    pub fn submit(&self) {
        todo!()
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
