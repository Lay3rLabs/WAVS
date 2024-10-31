use tokio::sync::mpsc;

use crate::apis::dispatcher::Service;
use crate::apis::submission::ChainMessage;
use crate::apis::trigger::TriggerAction;
use crate::context::AppContext;
use crate::engine::{Engine, EngineError};

use super::EngineRunner;

#[derive(Clone)]
pub struct SingleEngineRunner<E: Engine + Clone + 'static> {
    engine: E,
}

impl<E: Engine + Clone + 'static> SingleEngineRunner<E> {
    pub fn new(engine: E) -> Self {
        SingleEngineRunner { engine }
    }
}

impl<E: Engine + Clone + 'static> EngineRunner for SingleEngineRunner<E> {
    fn start(
        &self,
        _ctx: AppContext,
        mut input: mpsc::Receiver<(TriggerAction, Service)>,
        output: mpsc::Sender<ChainMessage>,
    ) -> Result<(), EngineError> {
        let _self = self.clone();
        std::thread::spawn(move || {
            while let Some((action, service)) = input.blocking_recv() {
                match _self.run_trigger(action, service) {
                    Ok(Some(msg)) => {
                        tracing::info!("Ran action, got result to submit");
                        if let Err(err) = output.blocking_send(msg) {
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
        });
        Ok(())
    }

    fn engine(&self) -> &dyn Engine {
        &self.engine
    }
}
