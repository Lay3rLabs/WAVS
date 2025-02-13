use rayon::ThreadPoolBuilder;
use tokio::sync::mpsc;
use tracing::instrument;
use wavs_types::{Service, TriggerAction};

use crate::apis::submission::ChainMessage;
use crate::engine::Engine;
use crate::AppContext;

use super::EngineRunner;

#[derive(Clone)]
pub struct MultiEngineRunner<E: Engine + Clone + 'static> {
    engine: E,
    thread_count: usize,
}

impl<E: Engine + Clone + 'static> MultiEngineRunner<E> {
    pub fn new(engine: E, thread_count: usize) -> Self {
        MultiEngineRunner {
            engine,
            thread_count,
        }
    }
}

impl<E: Engine + Clone + 'static> EngineRunner for MultiEngineRunner<E> {
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
            let pool = ThreadPoolBuilder::new()
                .num_threads(_self.thread_count)
                .build()
                .unwrap();
            while let Some((action, service)) = input.blocking_recv() {
                let runner = _self.clone();
                let result_sender = result_sender.clone();
                pool.install(move || {
                    if let Err(e) = runner.run_trigger(action, service, result_sender) {
                        tracing::error!("{:?}", e);
                    }
                })
            }
        });
    }

    fn engine(&self) -> &Self::Engine {
        &self.engine
    }
}
