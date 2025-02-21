use tokio::sync::mpsc;
use wavs_types::{Service, TriggerAction};

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

        let trigger_config = action.config.clone();

        let wasi_result =
            self.engine()
                .execute(component, workflow.fuel_limit, action, &service.config)?;

        if let Some(wasi_result) = wasi_result {
            let service_id = trigger_config.service_id.clone();
            let workflow_id = trigger_config.workflow_id.clone();

            let msg = ChainMessage {
                trigger_config,
                wasi_result,
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
