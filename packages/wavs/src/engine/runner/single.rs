use tokio::sync::mpsc;
use tracing::instrument;
use wavs_types::{Service, TriggerAction};

use crate::apis::submission::ChainMessage;
use crate::engine::Engine;
use crate::AppContext;

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
    type Engine = E;

    #[instrument(level = "debug", skip(self, _ctx), fields(subsys = "EngineRunner"))]
    fn start(
        &self,
        _ctx: AppContext,
        mut input: mpsc::Receiver<(TriggerAction, Service)>,
        result_sender: mpsc::Sender<ChainMessage>,
    ) {
        let _self = self.clone();

        std::thread::spawn(move || {
            while let Some((action, service)) = input.blocking_recv() {
                if let Err(e) = _self.run_trigger(action, service, result_sender.clone()) {
                    tracing::error!("{:?}", e);
                }
            }
        });
    }

    fn engine(&self) -> &Self::Engine {
        &self.engine
    }
}
