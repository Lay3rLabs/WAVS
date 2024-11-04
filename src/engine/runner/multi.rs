use rayon::ThreadPoolBuilder;
use tokio::sync::mpsc;

use crate::apis::dispatcher::Service;
use crate::apis::submission::ChainMessage;
use crate::apis::trigger::TriggerAction;
use crate::context::AppContext;
use crate::engine::{Engine, EngineError};

use super::EngineRunner;

// TODO: get from config
const DEFAULT_CHANNEL_SIZE: usize = 100;

#[derive(Clone)]
pub struct MultiEngineRunner<E: Engine + Clone + 'static> {
    engine: E,
    thread_count: usize,
    output_channel_size: usize,
}

impl<E: Engine + Clone + 'static> MultiEngineRunner<E> {
    pub fn new(engine: E, thread_count: usize) -> Self {
        MultiEngineRunner {
            engine,
            thread_count,
            output_channel_size: DEFAULT_CHANNEL_SIZE,
        }
    }
}

impl<E: Engine + Clone + 'static> EngineRunner for MultiEngineRunner<E> {
    type Engine = E;

    fn start(
        &self,
        _ctx: AppContext,
        mut input: mpsc::Receiver<(TriggerAction, Service)>,
    ) -> Result<mpsc::Receiver<ChainMessage>, EngineError> {
        let (output, rx) = mpsc::channel::<ChainMessage>(self.output_channel_size);
        let _self = self.clone();

        std::thread::spawn(move || {
            let pool = ThreadPoolBuilder::new()
                .num_threads(_self.thread_count)
                .build()
                .unwrap();
            while let Some((action, service)) = input.blocking_recv() {
                let runner = _self.clone();
                let out = output.clone();
                pool.install(move || match runner.run_trigger(action, service) {
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
                })
            }
        });
        Ok(rx)
    }

    fn engine(&self) -> &Self::Engine {
        &self.engine
    }
}
