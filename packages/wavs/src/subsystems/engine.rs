pub mod error;
pub mod wasm_engine;

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::FixedBytes;
use error::EngineError;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::storage::CAStorage;
use wavs_types::{
    ComponentDigest, Envelope, EventId, EventOrder, Service, TriggerAction, WorkflowId,
};

use crate::services::Services;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::submission::chain_message::ChainMessage;
use crate::AppContext;

pub struct EngineManager<S: CAStorage> {
    pub engine: Arc<WasmEngine<S>>,
    pub services: Services,
}

impl<S: CAStorage> Clone for EngineManager<S> {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
            services: self.services.clone(),
        }
    }
}

impl<S: CAStorage + Send + Sync + 'static> EngineManager<S> {
    pub fn new(engine: WasmEngine<S>, services: Services) -> Self {
        Self {
            engine: Arc::new(engine),
            services,
        }
    }

    #[instrument(skip(self, ctx), fields(subsys = "EngineRunner"))]
    pub fn start(
        &self,
        ctx: AppContext,
        mut input: mpsc::Receiver<(TriggerAction, Service)>,
        result_sender: mpsc::Sender<ChainMessage>,
    ) where
        S: 'static,
    {
        let _self = self.clone();

        std::thread::spawn(move || {
            while let Some((action, service)) = input.blocking_recv() {
                let _self = _self.clone();
                let result_sender = result_sender.clone();
                if let Err(e) = ctx
                    .rt
                    .block_on(_self.run_trigger(action, service, result_sender))
                {
                    tracing::error!("{:?}", e);
                }
            }
        });
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
        result_sender: mpsc::Sender<ChainMessage>,
    ) -> Result<(), EngineError> {
        // early-exit without an error if the service is not active
        if !self.services.is_active(&action.config.service_id) {
            tracing::info!(
                "Service is not active, skipping action: service_id={}",
                action.config.service_id
            );
            return Ok(());
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
            let service_id = trigger_config.service_id.clone();
            let workflow_id = trigger_config.workflow_id.clone();

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

            result_sender
                .send(msg)
                .await
                .map_err(|_| EngineError::WasiResultSend(service_id, workflow_id))
        } else {
            tracing::info!(
                "Component execution produced no result: service_id={}, workflow_id={}",
                trigger_config.service_id,
                trigger_config.workflow_id
            );
            Ok(())
        }
    }
}
