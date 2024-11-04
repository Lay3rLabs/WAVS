use tokio::sync::mpsc;

use crate::apis::dispatcher::{Service, Submit};
use crate::apis::submission::ChainMessage;
use crate::apis::trigger::{TriggerAction, TriggerResult};
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
            .get(&action.trigger.workflow_id)
            .ok_or_else(|| {
                EngineError::UnknownWorkflow(
                    action.trigger.service_id.clone(),
                    action.trigger.workflow_id.clone(),
                )
            })?;

        let component = service
            .components
            .get(&workflow.component)
            .ok_or_else(|| EngineError::UnknownComponent(workflow.component.clone()))?;

        // TODO: we actually get other info, like permissions and apply in the execution
        let digest = component.wasm.clone();

        match action.result {
            TriggerResult::Queue { task_id, payload } => {
                // TODO: add the timestamp to the trigger, don't invent it
                let timestamp = 1234567890;
                let wasm_result = self.engine().execute_queue(digest, payload, timestamp)?;

                // TODO: we need to sent these off to the submission engine
                if let Some(Submit::VerifierTx {
                    hd_index,
                    verifier_addr,
                }) = workflow.submit.as_ref()
                {
                    Ok(Some(ChainMessage {
                        trigger_data: action.trigger,
                        task_id,
                        wasm_result,
                        hd_index: *hd_index,
                        verifier_addr: verifier_addr.clone(),
                    }))
                } else {
                    Ok(None)
                }
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
            tracing::info!("Ran action, got result to submit");
            if let Err(err) = out.blocking_send(msg) {
                tracing::error!("Error submitting msg: {:?}", err);
            }
        }
        Ok(None) => {
            tracing::info!("Ran action, no submission");
        }
        Err(e) => {
            tracing::error!("Error running trigger: {:?}", e);
        }
    }
}
