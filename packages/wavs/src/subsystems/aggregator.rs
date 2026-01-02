pub mod error;

use tracing::instrument;
use utils::{context::AppContext, telemetry::AggregatorMetrics};
use wavs_types::AggregatorAction;

use crate::{
    config::Config,
    dispatcher::DispatcherCommand,
    services::Services,
    subsystems::{aggregator::error::AggregatorError, submission::data::Submission},
};

#[derive(Clone)]
pub struct Aggregator {
    pub metrics: AggregatorMetrics,
    services: Services,
    dispatcher_to_aggregator_rx: crossbeam::channel::Receiver<AggregatorCommand>,
    aggregator_to_self_tx: crossbeam::channel::Sender<AggregatorCommand>,
    subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
}

#[derive(Debug)]
pub enum AggregatorCommand {
    Kill,
    // From Submission Manager
    Broadcast(Submission),
    // From Peers and Self
    Receive(Submission),
    // From Engine Manager (right before sending on-chain)
    Submit {
        submission: Submission,
        actions: Vec<AggregatorAction>,
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
            dispatcher_to_aggregator_rx,
            aggregator_to_self_tx,
            subsystem_to_dispatcher_tx,
            metrics,
            services,
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

        while let Ok(msg) = self.dispatcher_to_aggregator_rx.recv() {
            match msg {
                AggregatorCommand::Kill => {
                    tracing::info!("Aggregator received Kill command, shutting down");
                    handle.abort();
                    break;
                }
                AggregatorCommand::Broadcast(submission) => {
                    let service = match _self.services.get(submission.service_id()) {
                        Ok(s) => s,
                        Err(e) => {
                            // this is an actual error, comes from submission manager
                            tracing::error!(
                                "Aggregator: Service not found for broadcast: {:?}",
                                submission.service_id()
                            );
                            continue;
                        }
                    };

                    ctx.rt.spawn({
                        let _self = _self.clone();
                        async move {
                            match _self.broadcast_submission(&submission).await {
                                Ok(_) => {
                                    _self.metrics.increment_broadcast_count(
                                        &service,
                                        submission.workflow_id(),
                                    );
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
                AggregatorCommand::Receive(submission) => {
                    let service = match _self.services.get(submission.service_id()) {
                        Ok(s) => s,
                        Err(e) => {
                            // this is NOT an error, could come from any peer
                            tracing::info!(
                                "Aggregator: Service not found for receive: {:?}",
                                submission.service_id()
                            );
                            continue;
                        }
                    };
                    ctx.rt.spawn({
                        let _self = _self.clone();
                        async move {
                            match _self.receive_submission(&submission).await {
                                Ok(_) => {
                                    _self.metrics.increment_receive_count(
                                        &service,
                                        submission.workflow_id(),
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Aggregator: Error receiving submission: {:?}",
                                        e
                                    );
                                }
                            }
                        }
                    });
                }

                AggregatorCommand::Submit {
                    submission,
                    actions,
                } => {
                    let service = match _self.services.get(submission.service_id()) {
                        Ok(s) => s,
                        Err(e) => {
                            // this is an actual error, comes from engine manager
                            tracing::error!(
                                "Aggregator: Service not found for submission: {:?}",
                                submission.service_id()
                            );
                            continue;
                        }
                    };
                    ctx.rt.spawn({
                        let _self = _self.clone();
                        async move {
                            match _self.submit_submission(&submission).await {
                                Ok(_) => {
                                    _self
                                        .metrics
                                        .increment_submit_count(&service, submission.workflow_id());
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Aggregator: Error submitting submission: {:?}",
                                        e
                                    );
                                }
                            }
                        }
                    });
                }
            }
        }

        ctx.rt.block_on(async move {
            match handle.await {
                Ok(_) => tracing::info!("Aggregator listener task completed successfully"),
                Err(e) => tracing::error!("Aggregator listener task failed: {:?}", e),
            }
        });
    }

    async fn handle_command(&self, command: AggregatorCommand) {
        match command {
            AggregatorCommand::Broadcast(submission) => {
                if let Err(e) = self.broadcast_submission(&submission).await {
                    tracing::error!("Failed to broadcast submission: {:?}", e);
                }
            }
            AggregatorCommand::Receive(submission) => {
                // Handle received submission
            }
            AggregatorCommand::Submit {
                submission,
                actions,
            } => {
                // Handle submission and actions
            }
            AggregatorCommand::Kill => {
                // Handle kill command if needed
            }
        }
    }

    async fn broadcast_submission(&self, submission: &Submission) -> Result<(), AggregatorError> {
        Ok(())
    }

    async fn receive_submission(&self, submission: &Submission) -> Result<(), AggregatorError> {
        Ok(())
    }

    async fn submit_submission(&self, submission: &Submission) -> Result<(), AggregatorError> {
        Ok(())
    }

    async fn start_listener(&self) {
        // Implement the logic to listen for incoming packets from the aggregator nodes
    }
}
/*
let mut submit_actions = BTreeSet::new();

loop {
    let mut new_actions = Vec::new();
    for aggregator_action in aggregator_actions.drain(..) {
        match aggregator_action {
            AggregatorAction::Submit(submit_action) => {
                submit_actions.insert(submit_action);
            }
            AggregatorAction::Timer(TimerAction { delay }) => {
                tokio::time::sleep(delay.into()).await;
                tracing::info!(
                    "Timer expired after {} seconds, executing callback for event {}",
                    delay.secs,
                    event_id
                );

                let actions = self
                    .engine
                    .execute_aggregator_component_timer_callback(
                        service.clone(),
                        trigger_action.clone(),
                        operator_response.clone(),
                    )
                    .await?;

                for aggregator_action in actions {
                    match aggregator_action {
                        AggregatorAction::Submit(submit_action) => {
                            submit_actions.insert(submit_action);
                        }
                        action => {
                            new_actions.push(action);
                        }
                    }
                }
            }
        }
    }

    if new_actions.is_empty() {
        break;
    }

    aggregator_actions = new_actions;
}

Ok(submit_actions.into_iter().collect())
*/
