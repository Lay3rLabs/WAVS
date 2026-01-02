pub mod error;
pub mod wasm_engine;

use std::collections::HashMap;
use std::sync::Arc;

use error::EngineError;
use tracing::instrument;
use utils::storage::CAStorage;
use wavs_types::{AggregatorAction, ComponentDigest, Service, TriggerAction, WorkflowId};

use crate::dispatcher::DispatcherCommand;
use crate::services::Services;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::submission::data::{Submission, SubmissionRequest};
use crate::AppContext;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineCommand {
    Kill,
    ExecuteOperator {
        action: TriggerAction,
        service: Service,
    },
    ExecuteAggregator {
        submission: Submission,
        service: Service,
    },
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineResponse {
    Operator(SubmissionRequest),
    Aggregator {
        submission: Submission,
        actions: Vec<AggregatorAction>,
    },
}

#[derive(Clone)]
pub struct EngineManager<S: CAStorage> {
    pub engine: Arc<WasmEngine<S>>,
    pub services: Services,
    pub dispatcher_to_engine_rx: crossbeam::channel::Receiver<EngineCommand>,
    pub subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
}

impl<S: CAStorage + Send + Sync + 'static> EngineManager<S> {
    pub fn new(
        engine: WasmEngine<S>,
        services: Services,
        dispatcher_to_engine_rx: crossbeam::channel::Receiver<EngineCommand>,
        subsystem_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    ) -> Self {
        Self {
            engine: Arc::new(engine),
            services,
            dispatcher_to_engine_rx,
            subsystem_to_dispatcher_tx,
        }
    }

    #[instrument(skip(self, ctx), fields(subsys = "EngineRunner"))]
    pub fn start(&self, ctx: AppContext)
    where
        S: 'static,
    {
        while let Ok(command) = self.dispatcher_to_engine_rx.recv() {
            match command {
                EngineCommand::Kill => {
                    tracing::info!("Received kill command, shutting down engine manager");
                    break;
                }
                EngineCommand::ExecuteOperator { action, service } => {
                    let _self = self.clone();
                    ctx.rt.spawn(async move {
                        match _self.run_trigger(action, service).await {
                            Err(e) => {
                                tracing::error!("Error running trigger: {:?}", e);
                            }
                            Ok(messages) => {
                                for msg in messages {
                                    if let Err(e) = _self.subsystem_to_dispatcher_tx.send(
                                        DispatcherCommand::EngineResponse(
                                            EngineResponse::Operator(msg),
                                        ),
                                    ) {
                                        tracing::error!(
                                            "Error sending message to dispatcher: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    });
                }
                EngineCommand::ExecuteAggregator {
                    submission,
                    service,
                } => {
                    let _self = self.clone();
                    ctx.rt.spawn(async move {
                        match _self.run_aggregator(&submission, service).await {
                            Err(e) => {
                                tracing::error!("Error running trigger: {:?}", e);
                            }
                            Ok(actions) => {
                                if let Err(e) = _self.subsystem_to_dispatcher_tx.send(
                                    DispatcherCommand::EngineResponse(EngineResponse::Aggregator {
                                        submission,
                                        actions,
                                    }),
                                ) {
                                    tracing::error!("Error sending message to dispatcher: {:?}", e);
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    #[instrument(skip(self), fields(subsys = "Engine"))]
    pub async fn store_components_for_service(
        &self,
        service: &Service,
    ) -> Result<HashMap<WorkflowId, ComponentDigest>, EngineError> {
        let mut digests = HashMap::new();

        for (workflow_id, workflow) in service.workflows.iter() {
            let digest = self
                .engine
                .store_component_from_source(&workflow.component.source)
                .await?;
            digests.insert(workflow_id.clone(), digest);
        }

        Ok(digests)
    }

    async fn run_trigger(
        &self,
        action: TriggerAction,
        service: Service,
    ) -> Result<Vec<SubmissionRequest>, EngineError> {
        // early-exit without an error if the service is not active
        if !self.services.is_active(&action.config.service_id) {
            tracing::info!(
                "Service is not active, skipping action: service_id={}",
                action.config.service_id
            );
            return Ok(Vec::new());
        }
        // early-exit if we can't get the workflow
        let workflow = service
            .workflows
            .get(&action.config.workflow_id)
            .ok_or_else(|| {
                EngineError::UnknownWorkflow(
                    action.config.service_id.clone(),
                    action.config.workflow_id.clone(),
                )
            })?;

        let trigger_config = action.config.clone();

        tracing::info!(
            "Executing component: service_id={}, workflow_id={}, component_digest={:?}",
            trigger_config.service_id,
            trigger_config.workflow_id,
            workflow.component.source.digest()
        );

        let mut wasm_responses = self
            .engine
            .execute_operator_component(service.clone(), action.clone())
            .await?;

        let mut submission_datas = Vec::new();
        // if there are results, send them down the pipeline to the submit processor
        // otherwise, just end early here, performing no action (but updating local state if needed)
        if wasm_responses.is_empty() {
            tracing::info!(
                service_id = %trigger_config.service_id,
                service.name = %service.name,
                service.manager = ?service.manager,
                workflow_id = %trigger_config.workflow_id,
                "Service {} (workflow {}) component execution produced no result",
                service.name,
                trigger_config.workflow_id
            );
        } else {
            for operator_response in wasm_responses.drain(..) {
                let submission_data = SubmissionRequest {
                    trigger_action: action.clone(),
                    operator_response,
                    service: service.clone(),
                    #[cfg(feature = "dev")]
                    debug: Default::default(),
                };

                let event_id = submission_data
                    .event_id()
                    .map_err(EngineError::EncodeEventId)?;
                let payload_size = submission_data.operator_response.payload.len();

                tracing::info!(
                    service_id = %trigger_config.service_id,
                    service.name = %service.name,
                    service.manager = ?service.manager,
                    workflow_id = %trigger_config.workflow_id,
                    payload_size = %payload_size,
                    event_id = %event_id,
                    "Service {} (workflow {}) component execution completed",
                    service.name,
                    trigger_config.workflow_id
                );

                submission_datas.push(submission_data);
            }
        }
        Ok(submission_datas)
    }

    async fn run_aggregator(
        &self,
        Submission {
            trigger_action,
            operator_response,
            event_id,
            ..
        }: &Submission,
        service: Service,
    ) -> Result<Vec<AggregatorAction>, EngineError> {
        let aggregator_actions = self
            .engine
            .execute_aggregator_component(
                service.clone(),
                trigger_action.clone(),
                operator_response.clone(),
            )
            .await?;

        if aggregator_actions.is_empty() {
            tracing::info!(
                service_id = %trigger_action.config.service_id,
                service.name = %service.name,
                service.manager = ?service.manager,
                workflow_id = %trigger_action.config.workflow_id,
                event_id = %event_id,
                "Service {} (workflow {}) aggregator execution produced no result",
                service.name,
                trigger_action.config.workflow_id
            );
        } else {
            tracing::info!(
                service_id = %trigger_action.config.service_id,
                service.name = %service.name,
                service.manager = ?service.manager,
                workflow_id = %trigger_action.config.workflow_id,
                event_id = %event_id,
                "Service {} (workflow {}) aggregator execution completed with {} actions",
                service.name,
                trigger_action.config.workflow_id,
                aggregator_actions.len()
            );
        }

        Ok(aggregator_actions)
    }
}
