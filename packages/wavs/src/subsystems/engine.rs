pub mod error;
pub mod wasm_engine;

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::FixedBytes;
use error::EngineError;
use tracing::instrument;
use utils::storage::CAStorage;
use wavs_types::{
    ComponentDigest, Envelope, EventId, EventIdSalt, EventOrder, Service, TriggerAction, WorkflowId,
};

use crate::services::Services;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::submission::chain_message::ChainMessage;
use crate::AppContext;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineCommand {
    Kill,
    Execute {
        action: TriggerAction,
        service: Service,
    },
}

#[derive(Clone)]
pub struct EngineManager<S: CAStorage> {
    pub engine: Arc<WasmEngine<S>>,
    pub services: Services,
    pub dispatcher_to_engine_rx: crossbeam::channel::Receiver<EngineCommand>,
    pub engine_to_dispatcher_tx: crossbeam::channel::Sender<ChainMessage>,
}

impl<S: CAStorage + Send + Sync + 'static> EngineManager<S> {
    pub fn new(
        engine: WasmEngine<S>,
        services: Services,
        dispatcher_to_engine_rx: crossbeam::channel::Receiver<EngineCommand>,
        engine_to_dispatcher_tx: crossbeam::channel::Sender<ChainMessage>,
    ) -> Self {
        Self {
            engine: Arc::new(engine),
            services,
            dispatcher_to_engine_rx,
            engine_to_dispatcher_tx,
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
                EngineCommand::Execute { action, service } => {
                    let _self = self.clone();
                    ctx.rt.spawn(async move {
                        match _self.run_trigger(action, service).await {
                            Err(e) => {
                                tracing::error!("Error running trigger: {:?}", e);
                            }
                            Ok(messages) => {
                                for msg in messages {
                                    #[cfg(feature = "rerun")]
                                    wavs_rerun::log_packet_flow(
                                        wavs_rerun::NODE_ENGINE,
                                        wavs_rerun::NODE_DISPATCHER,
                                        &msg.envelope.eventId.to_string(),
                                        &msg.workflow_id.to_string(),
                                        None,
                                    );

                                    if let Err(e) = _self.engine_to_dispatcher_tx.send(msg) {
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
    ) -> Result<Vec<ChainMessage>, EngineError> {
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

        let wasm_responses = self.engine.execute(service.clone(), action.clone()).await?;

        let mut messages = Vec::new();
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
            for wasm_response in wasm_responses {
                let event_id = match wasm_response.event_id_salt {
                    Some(salt) => EventId::new(
                        &service.id(),
                        &trigger_config.workflow_id,
                        EventIdSalt::WasmResponse(&salt),
                    )
                    .map_err(EngineError::EncodeEventId)?,
                    None => EventId::new(
                        &service.id(),
                        &trigger_config.workflow_id,
                        EventIdSalt::Trigger(&action.data),
                    )
                    .map_err(EngineError::EncodeEventId)?,
                };
                tracing::info!(
                    service_id = %trigger_config.service_id,
                    service.name = %service.name,
                    service.manager = ?service.manager,
                    workflow_id = %trigger_config.workflow_id,
                    payload_size = %wasm_response.payload.len(),
                    event_id = %event_id,
                    "Service {} (workflow {}) component execution completed",
                    service.name,
                    trigger_config.workflow_id
                );

                let msg = ChainMessage {
                    service_id: trigger_config.service_id.clone(),
                    workflow_id: trigger_config.workflow_id.clone(),
                    envelope: Envelope {
                        payload: wasm_response.payload.into(),
                        eventId: event_id.into(),
                        ordering: match wasm_response.ordering {
                            Some(ordering) => EventOrder::new_u64(ordering).into(),
                            None => FixedBytes::default(),
                        },
                    },
                    submit: workflow.submit.clone(),
                    #[cfg(feature = "dev")]
                    debug: Default::default(),
                    trigger_data: action.data.clone(),
                };

                messages.push(msg);
            }
        }
        Ok(messages)
    }
}
