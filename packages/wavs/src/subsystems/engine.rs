pub mod error;
pub mod wasm_engine;

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::FixedBytes;
use error::EngineError;
use tracing::instrument;
use utils::storage::CAStorage;
use wavs_types::{
    ComponentDigest, Envelope, EventId, EventOrder, Service, TriggerAction, WorkflowId,
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
                            Ok(Some(msg)) => {
                                if let Err(e) = _self.engine_to_dispatcher_tx.send(msg) {
                                    tracing::error!("Error sending message to dispatcher: {:?}", e);
                                }
                            }
                            Ok(None) => {
                                // No message to send, just continue
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
    ) -> Result<Option<ChainMessage>, EngineError> {
        // early-exit without an error if the service is not active
        if !self.services.is_active(&action.config.service_id) {
            tracing::info!(
                "Service is not active, skipping action: service_id={}",
                action.config.service_id
            );
            return Ok(None);
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

        let wasm_response = self.engine.execute(service.clone(), action.clone()).await?;

        // If Ok(Some(x)), send the result down the pipeline to the submit processor
        // If Ok(None), just end early here, performing no action (but updating local state if needed)
        if let Some(wasm_response) = wasm_response {
            tracing::info!(service.name = %service.name, service.manager = ?service.manager, workflow_id = %trigger_config.workflow_id, payload_size = %wasm_response.payload.len(), "Component execution produced result: service={} [{:?}], workflow_id={}, payload_size={}", service.name, service.manager, trigger_config.workflow_id, wasm_response.payload.len());

            let msg = ChainMessage {
                service_id: trigger_config.service_id,
                workflow_id: trigger_config.workflow_id,
                envelope: Envelope {
                    payload: wasm_response.payload.into(),
                    eventId: EventId::try_from((&service, &action))
                        .map_err(EngineError::EncodeEventId)?
                        .into(),
                    ordering: match wasm_response.ordering {
                        Some(ordering) => EventOrder::new_u64(ordering).into(),
                        None => FixedBytes::default(),
                    },
                },
                submit: workflow.submit.clone(),
                #[cfg(debug_assertions)]
                debug: Default::default(),
                trigger_data: action.data,
            };

            Ok(Some(msg))
        } else {
            tracing::info!(
                "Component execution produced no result: service_id={}, workflow_id={}",
                trigger_config.service_id,
                trigger_config.workflow_id
            );
            Ok(None)
        }
    }
}
