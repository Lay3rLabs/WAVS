use tokio::sync::mpsc;
use wavs_types::{Envelope, EventId, EventOrder, PacketRoute, Service, TriggerAction};

use crate::apis::engine::ExecutionComponent;
use crate::apis::submission::ChainMessage;
use crate::engine::{Engine, EngineError};
use crate::AppContext;

pub trait EngineRunner: Send + Sync {
    type Engine: Engine;

    // This starts a loop to process all incoming triggers ans prepare outgoing results
    // It should immediately return and run the processing task in the background
    fn start(
        &self,
        ctx: AppContext,
        input: mpsc::Receiver<(TriggerAction, Service)>,
        result_sender: mpsc::Sender<ChainMessage>,
    );

    // Return the engine if they want to use that directly.
    fn engine(&self) -> &Self::Engine;

    /// This is where the heavy lifting is done (at least for now, where self.engine.execute_queue happens in the same thread)
    /// effectively, it slows down the consumption of triggers and can inadvertendly cause the whole system to slow down
    fn run_trigger(
        &self,
        action: TriggerAction,
        service: Service,
        result_sender: mpsc::Sender<ChainMessage>,
    ) -> Result<(), EngineError> {
        // look up the proper workflow
        let workflow = service
            .workflows
            .get(&action.config.workflow_id)
            .ok_or_else(|| {
                EngineError::UnknownWorkflow(
                    action.config.service_id.clone(),
                    action.config.workflow_id.clone(),
                )
            })?;

        let component = service
            .components
            .get(&workflow.component)
            .ok_or_else(|| EngineError::UnknownComponent(workflow.component.clone()))?;

        let digest = match &component.source {
            wavs_types::ComponentSource::Download { digest, .. } => digest,
            wavs_types::ComponentSource::Registry { registry } => &registry.digest,
            wavs_types::ComponentSource::Digest(digest) => digest,
        };

        let execution_component = ExecutionComponent {
            wasm: digest.clone(),
            permissions: component.permissions.clone(),
        };

        let trigger_config = action.config.clone();

        let wasi_result = self.engine().execute(
            &execution_component,
            workflow.fuel_limit,
            action.clone(),
            &service.config,
        )?;

        // If Ok(Some(x)), send the result down the pipeline to the submit processor
        // If Ok(None), just end early here, performing no action (but updating local state if needed)
        if let Some(wasi_result) = wasi_result {
            let service_id = trigger_config.service_id.clone();
            let workflow_id = trigger_config.workflow_id.clone();

            let msg = ChainMessage {
                packet_route: PacketRoute::new_trigger_config(&trigger_config),
                envelope: Envelope {
                    payload: wasi_result.into(),
                    eventId: EventId::try_from(&action)
                        .map_err(EngineError::EncodeEventId)?
                        .into(),
                    ordering: EventOrder::try_from(&action)
                        .map_err(EngineError::EncodeEventOrder)?
                        .into(),
                },
                submit: workflow.submit.clone(),
            };

            result_sender
                .blocking_send(msg)
                .map_err(|_| EngineError::WasiResultSend(service_id, workflow_id))
        } else {
            Ok(())
        }
    }
}
