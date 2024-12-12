use tokio::sync::mpsc;

use crate::apis::dispatcher::Service;
use crate::apis::submission::ChainMessage;
use crate::apis::trigger::{TriggerAction, TriggerData};
use crate::context::AppContext;
use crate::engine::{Engine, EngineError};

pub trait EngineRunner: Send + Sync {
    type Engine: Engine;

    // This starts a loop to process all incoming triggers ans prepare outgoing results
    // It should immediately return and run the processing task in the background
    fn start(
        &self,
        ctx: AppContext,
        input: mpsc::Receiver<(TriggerAction, Service)>,
    ) -> Result<mpsc::Receiver<ChainMessage>, EngineError>;

    // Return the engine if they want to use that directly.
    fn engine(&self) -> &Self::Engine;

    /// This is where the heavy lifting is done (at least for now, where self.engine.execute_queue happens in the same thread)
    /// effectively, it slows down the consumption of triggers and can inadvertendly cause the whole system to slow down
    fn run_trigger(
        &self,
        action: TriggerAction,
        service: Service,
    ) -> Result<Option<ChainMessage>, EngineError> {
        // look up the proper workflow
        let workflow = service
            .workflows
            .get(&action.trigger_config.workflow_id)
            .ok_or_else(|| {
                EngineError::UnknownWorkflow(
                    action.trigger_config.service_id.clone(),
                    action.trigger_config.workflow_id.clone(),
                )
            })?;

        let component = service
            .components
            .get(&workflow.component)
            .ok_or_else(|| EngineError::UnknownComponent(workflow.component.clone()))?;

        match action.result {
            TriggerData::Queue { task_id, payload } => {
                // TODO: add the timestamp to the trigger, don't invent it
                let timestamp = 1234567890;
                let wasm_result =
                    self.engine()
                        .execute_queue(component, &service.id, payload, timestamp)?;

                Ok(workflow.submit.clone().map(|submit| ChainMessage {
                    trigger_config: action.trigger_config,
                    task_id,
                    wasm_result,
                    submit,
                }))
            }
        }
    }
}

pub fn submit_result(
    out: &mpsc::Sender<ChainMessage>,
    msg: Result<Option<ChainMessage>, EngineError>,
) {
    match msg {
        Ok(Some(msg)) => {
            tracing::debug!("Ran action, got result to submit");
            if let Err(err) = out.blocking_send(msg) {
                tracing::error!("Error submitting msg: {:?}", err);
            }
        }
        Ok(None) => {
            tracing::debug!("Ran action, no submission");
        }
        Err(e) => {
            tracing::error!("Error running trigger: {:?}", e);
        }
    }
}
