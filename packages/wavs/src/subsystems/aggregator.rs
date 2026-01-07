pub mod error;
pub mod peer;
mod queue;
mod submit;

use std::{collections::HashMap, sync::Arc};

use layer_climb::prelude::*;
use tracing::instrument;
use utils::{
    async_transaction::AsyncTransaction, config::EvmChainConfigExt, context::AppContext,
    evm_client::EvmSigningClient, storage::db::WavsDb, telemetry::AggregatorMetrics,
};
use wavs_engine::bindings::aggregator::world::AnyTxHash;
use wavs_types::{
    AggregatorAction, ChainKey, QuorumQueue, QuorumQueueId, Service, Submission, Submit,
    SubmitAction, TimerAction,
};

use crate::{
    config::Config,
    dispatcher::DispatcherCommand,
    services::Services,
    subsystems::{
        aggregator::{
            error::AggregatorError, peer::Peer, queue::append_submission_to_queue,
            submit::AnyTransactionReceipt,
        },
        engine::AggregatorExecuteKind,
    },
};

#[derive(Clone)]
pub struct Aggregator {
    pub metrics: AggregatorMetrics,
    storage: WavsDb,
    config: Arc<Config>,
    services: Services,
    dispatcher_to_aggregator_rx: crossbeam::channel::Receiver<AggregatorCommand>,
    aggregator_to_self_tx: crossbeam::channel::Sender<AggregatorCommand>,
    subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    evm_submission_clients: Arc<std::sync::RwLock<HashMap<ChainKey, EvmSigningClient>>>,
    cosmos_submission_clients:
        Arc<std::sync::RwLock<HashMap<ChainKey, layer_climb::prelude::SigningClient>>>,
    queue_transaction: AsyncTransaction<QuorumQueueId>,
    chain_transaction: AsyncTransaction<ChainKey>,
}

#[derive(Debug)]
pub enum AggregatorCommand {
    Kill,
    // From Submission Manager
    Broadcast(Submission),
    // From Peers and Self
    Receive {
        submission: Submission,
        peer: Peer,
    },
    // From Engine Manager (right before sending on-chain)
    Actions {
        submission: Submission,
        actions: Vec<AggregatorAction>,
        kind: AggregatorExecuteKind,
    },
}

impl Aggregator {
    #[allow(clippy::new_without_default)]
    #[instrument(skip(services), fields(subsys = "Aggregator"))]
    pub fn new(
        config: &Config,
        metrics: AggregatorMetrics,
        services: Services,
        dispatcher_to_aggregator_rx: crossbeam::channel::Receiver<AggregatorCommand>,
        aggregator_to_self_tx: crossbeam::channel::Sender<AggregatorCommand>,
        subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    ) -> Result<Self, AggregatorError> {
        Ok(Self {
            storage: WavsDb::new().map_err(AggregatorError::Db)?,
            dispatcher_to_aggregator_rx,
            aggregator_to_self_tx,
            subsystem_to_dispatcher_tx,
            metrics,
            services,
            evm_submission_clients: Arc::new(std::sync::RwLock::new(HashMap::default())),
            cosmos_submission_clients: Arc::new(std::sync::RwLock::new(HashMap::default())),
            config: Arc::new(config.clone()),
            queue_transaction: AsyncTransaction::new(false),
            chain_transaction: AsyncTransaction::new(false),
        })
    }

    #[instrument(skip(self, ctx), fields(subsys = "Aggregator"))]
    pub fn start(&self, ctx: AppContext) {
        let _self = self.clone();

        let handle = ctx.rt.spawn({
            let _self = self.clone();
            async move {
                _self.start_listener().await;
            }
        });

        while let Ok(command) = self.dispatcher_to_aggregator_rx.recv() {
            self.handle_dispatcher_command(&ctx, command);

            if ctx.killed() {
                break;
            }
        }

        handle.abort();

        ctx.rt.block_on(async move {
            match handle.await {
                Ok(_) => tracing::info!("Aggregator listener task completed successfully"),
                Err(e) => tracing::error!("Aggregator listener task failed: {:?}", e),
            }
        });
    }

    fn handle_dispatcher_command(&self, ctx: &AppContext, command: AggregatorCommand) {
        let label = match &command {
            AggregatorCommand::Kill => "Kill".to_string(),
            AggregatorCommand::Broadcast(submission) => {
                format!("{} Broadcast", submission.label())
            }
            AggregatorCommand::Receive { submission, peer } => {
                format!("{} Receive from peer: {:?}", submission.label(), peer)
            }
            AggregatorCommand::Actions {
                submission,
                actions,
                kind,
            } => format!(
                "{} {} Actions (count: {})",
                submission.label(),
                match kind {
                    AggregatorExecuteKind::Standard => "Standard",
                    AggregatorExecuteKind::SubmitCallback { .. } => "SubmitCallback",
                    AggregatorExecuteKind::TimerCallback => "TimerCallback",
                },
                actions.len()
            ),
        };

        tracing::info!("Aggregator received command: {}", label);

        match command {
            AggregatorCommand::Kill => {
                tracing::info!("Aggregator received Kill command, shutting down");
            }
            AggregatorCommand::Broadcast(submission) => {
                let service =
                    match self.extract_service_from_submission(&submission, &Peer::Me, &label) {
                        Some(s) => s,
                        None => {
                            return;
                        }
                    };

                ctx.rt.spawn({
                    let _self = self.clone();
                    async move {
                        match _self.handle_broadcast(&submission).await {
                            Ok(_) => {
                                _self
                                    .metrics
                                    .increment_broadcast_count(&service, submission.workflow_id());
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Aggregator: Error broadcasting submission: {:?}",
                                    e
                                );
                            }
                        }
                    }
                });
            }
            AggregatorCommand::Receive { submission, peer } => {
                let service = match self.extract_service_from_submission(&submission, &peer, &label)
                {
                    Some(s) => s,
                    None => {
                        return;
                    }
                };
                ctx.rt.spawn({
                    let _self = self.clone();
                    async move {
                        let workflow_id = submission.workflow_id().clone();

                        match _self.handle_receive(submission, service.clone()).await {
                            Ok(_) => {
                                _self
                                    .metrics
                                    .increment_receive_count(&service, &workflow_id);
                            }
                            Err(e) => {
                                tracing::error!("Aggregator: Error receiving submission: {:?}", e);
                            }
                        }
                    }
                });
            }

            AggregatorCommand::Actions {
                submission,
                actions,
                kind: _,
            } => {
                let service =
                    match self.extract_service_from_submission(&submission, &Peer::Me, &label) {
                        Some(s) => s,
                        None => {
                            return;
                        }
                    };

                for action in actions {
                    ctx.rt.spawn({
                        let _self = self.clone();
                        let service = service.clone();
                        let submission = submission.clone();
                        async move {
                            match action {
                                AggregatorAction::Submit(action) => {
                                    let workflow_id = submission.workflow_id().clone();
                                    let queue_id = QuorumQueueId {
                                        event_id: submission.event_id.clone(),
                                        action: action.clone(),
                                    };
                                    // other queue ids can run concurrently, but this makes sure that
                                    // we lock this queue_id against changes from other requests coming in while we process it
                                    _self
                                        .queue_transaction
                                        .run(queue_id.clone(), {
                                            let _self = _self.clone();
                                            move || async move {
                                                let mut queue = match _self.get_quorum_queue(&queue_id).await {
                                                    Ok(queue) => {
                                                        match queue {
                                                            QuorumQueue::Burned => {
                                                                tracing::warn!("Tried to access burned quorum queue: {:?}", queue_id);
                                                                return;
                                                            }
                                                            QuorumQueue::Active(submissions) => submissions,
                                                        }
                                                    }
                                                    Err(err) => {
                                                        tracing::error!(
                                                            "Aggregator: Error getting quorum queue {:?}: {:?}",
                                                            queue_id,
                                                            err
                                                        );
                                                        return;
                                                    },
                                                };

                                                // This is pushed into the queue temporarily for signature aggregation
                                                // but will only be saved to the current queue if we get a "InsufficientQuorum" error
                                                if let Err(err) = append_submission_to_queue(&queue_id, &mut queue,submission.clone()) {
                                                    tracing::error!("{}", err);
                                                    return;
                                                }


                                                tracing::info!("Queue count for {:?}: {}", queue_id, queue.len());

                                                match _self
                                                    .handle_submit_action(
                                                        &submission,
                                                        &service,
                                                        queue_id,
                                                        queue,
                                                        action,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        _self.metrics.increment_action_count(
                                                            &service,
                                                            &workflow_id,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Aggregator: Error handling actions: {:?}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        })
                                        .await;
                                },
                                AggregatorAction::Timer(TimerAction { delay }) => {
                                    // this is fine, we're in our own spawned task
                                    tokio::time::sleep(delay.into()).await;
                                    tracing::info!(
                                        "Timer expired after {} seconds, executing callback for event {}",
                                        delay.secs,
                                        submission.event_id
                                    );

                                    if let Err(e) = _self.subsystem_to_dispatcher_tx
                                        .send(DispatcherCommand::AggregatorExecute {
                                            submission: submission.clone(),
                                            service: service.clone(),
                                            kind: AggregatorExecuteKind::TimerCallback,
                                        }) {
                                            tracing::error!(
                                                "Aggregator: Error sending Timer callback to Dispatcher: {:?}",
                                                e
                                            );
                                        }

                                }
                            }
                        }
                    });
                }
            }
        }
    }

    fn extract_service_from_submission(
        &self,
        submission: &Submission,
        peer: &Peer,
        label: &str,
    ) -> Option<Service> {
        match self.services.get(submission.service_id()) {
            Ok(service) => {
                let workflow = match service.workflows.get(submission.workflow_id()) {
                    Some(w) => w,
                    None => {
                        tracing::error!("{label}: Workflow not found",);
                        return None;
                    }
                };

                if !matches!(workflow.submit, Submit::Aggregator { .. }) {
                    tracing::info!(
                        "{label}: Received submission for workflow not using Aggregator submission",
                    );
                    return None;
                }

                Some(service)
            }
            Err(_) => {
                match &peer {
                    Peer::Me => {
                        // this IS an error, we should never receive submissions from ourself for services we don't host
                        tracing::error!("{label}: Service not found for receive from myself",);
                    }
                    Peer::Other(_) => {
                        // this is NOT an error, peers can broadcast submissions for services we don't host
                        tracing::info!("{label}: Service not found for receive from peer",);
                    }
                }
                None
            }
        }
    }

    async fn handle_broadcast(&self, submission: &Submission) -> Result<(), AggregatorError> {
        self.aggregator_to_self_tx
            .send(AggregatorCommand::Receive {
                submission: submission.clone(),
                peer: Peer::Me,
            })
            .map_err(Box::new)?;

        Ok(())
    }

    async fn handle_receive(
        &self,
        submission: Submission,
        service: Service,
    ) -> Result<(), AggregatorError> {
        // TODO - broadcast to peers

        self.subsystem_to_dispatcher_tx
            .send(DispatcherCommand::AggregatorExecute {
                submission,
                service,
                kind: AggregatorExecuteKind::Standard,
            })
            .map_err(Box::new)?;

        Ok(())
    }

    async fn handle_submit_action(
        &self,
        submission: &Submission,
        service: &Service,
        queue_id: QuorumQueueId,
        queue: Vec<Submission>,
        action: SubmitAction,
    ) -> Result<(), AggregatorError> {
        // running in a transaction keyed by chain to avoid nonce errors
        let result: Result<Option<AnyTransactionReceipt>, AggregatorError> = self
            .chain_transaction
            .run(action.chain().clone(), {
                let _self = self.clone();
                let queue = queue.clone();
                move || async move {
                    match action {
                        SubmitAction::Evm(action) => {
                            let client = match _self.get_evm_client(&action.chain).await? {
                                Some(c) => c,
                                None => {
                                    return Ok(None);
                                }
                            };

                            _self
                                .handle_action_submit_evm(client, &queue, action)
                                .await
                                .map(Some)
                        }
                        SubmitAction::Cosmos(action) => {
                            let client = match _self.get_cosmos_client(&action.chain).await? {
                                Some(c) => c,
                                None => {
                                    return Ok(None);
                                }
                            };

                            _self
                                .handle_action_submit_cosmos(client, &queue, action)
                                .await
                                .map(Some)
                        }
                    }
                }
            })
            .await;

        // just mapping the result to handle the Option
        // and returning early if None
        let result = match result {
            Ok(None) => {
                return Ok(());
            }
            Ok(Some(tx_resp)) => Ok(tx_resp),
            Err(e) => Err(e),
        };

        match &result {
            Err(AggregatorError::InsufficientQuorum {
                signer_weight,
                threshold_weight,
                total_weight,
            }) => {
                tracing::warn!(
                        "Aggregator: Insufficient quorum for {}: signer weight: {}, threshold weight: {}, total weight: {}",
                        submission.label(),
                        signer_weight,
                        threshold_weight,
                        total_weight
                    );

                self.save_quorum_queue(queue_id, queue).await?;
            }
            Ok(tx_resp) => {
                tracing::info!(
                    "Aggregator: Successfully submitted on-chain for {}: tx hash: {}",
                    submission.label(),
                    tx_resp.tx_hash()
                );
                self.burn_quorum_queue(queue_id).await?;
            }

            Err(err) => {
                tracing::error!(
                    "Aggregator: Error submitting on-chain for submission {}: {:?}",
                    submission.label(),
                    err
                );
            }
        }

        self.subsystem_to_dispatcher_tx
            .send(DispatcherCommand::AggregatorExecute {
                submission: submission.clone(),
                service: service.clone(),
                kind: AggregatorExecuteKind::SubmitCallback {
                    result: result
                        .map(|tx_resp| match tx_resp {
                            AnyTransactionReceipt::Evm(transaction_receipt) => {
                                AnyTxHash::Evm(transaction_receipt.transaction_hash.to_vec())
                            }
                            AnyTransactionReceipt::Cosmos(tx_hash) => AnyTxHash::Cosmos(tx_hash),
                        })
                        .map_err(|err| err.to_string()),
                },
            })
            .map_err(Box::new)?;

        Ok(())
    }

    async fn start_listener(&self) {
        // Implement the logic to listen for incoming packets from the aggregator nodes
    }

    async fn get_evm_client(
        &self,
        chain: &ChainKey,
    ) -> Result<Option<EvmSigningClient>, AggregatorError> {
        {
            let client = self
                .evm_submission_clients
                .read()
                .unwrap()
                .get(chain)
                .cloned();

            if let Some(client) = client {
                return Ok(Some(client));
            }
        };

        let credential = match &self.config.aggregator_evm_credential {
            Some(credential) => credential,
            None => {
                tracing::warn!("Aggregator: Missing EVM credential for chain: {}", chain);
                return Ok(None);
            }
        };

        let chain_config = match self.config.chains.read().unwrap().get_chain(chain) {
            Some(chain_config) => chain_config.to_evm_config()?,
            None => {
                tracing::warn!("Aggregator: Chain config not found for chain: {}", chain);
                return Ok(None);
            }
        };

        let client_config = chain_config.signing_client_config(credential.clone())?;

        let client = EvmSigningClient::new(client_config)
            .await
            .map_err(AggregatorError::CreateEvmClient)?;

        {
            let clients = &mut self.evm_submission_clients.write().unwrap();
            clients.insert(chain.clone(), client.clone());
        }

        Ok(Some(client))
    }

    async fn get_cosmos_client(
        &self,
        chain: &ChainKey,
    ) -> Result<Option<layer_climb::prelude::SigningClient>, AggregatorError> {
        {
            let client = self
                .cosmos_submission_clients
                .read()
                .unwrap()
                .get(chain)
                .cloned();

            if let Some(client) = client {
                return Ok(Some(client));
            }
        };

        let credential = match &self.config.aggregator_cosmos_credential {
            Some(credential) => credential,
            None => {
                tracing::warn!("Aggregator: Missing Cosmos credential for chain: {}", chain);
                return Ok(None);
            }
        };

        let chain_config = match self.config.chains.read().unwrap().get_chain(chain) {
            Some(chain_config) => chain_config.to_cosmos_config()?,
            None => {
                tracing::warn!("Aggregator: Chain config not found for chain: {}", chain);
                return Ok(None);
            }
        };

        let key_signer =
            KeySigner::new_mnemonic_str(credential, None).map_err(AggregatorError::CosmosClient)?;

        let client = SigningClient::new(chain_config.into(), key_signer, None)
            .await
            .map_err(AggregatorError::CosmosClient)?;

        {
            let clients = &mut self.cosmos_submission_clients.write().unwrap();
            clients.insert(chain.clone(), client.clone());
        }

        Ok(Some(client))
    }
}

impl Drop for Aggregator {
    fn drop(&mut self) {
        tracing::warn!("Dropping Aggregator subsystem");
    }
}
