use std::sync::Arc;

use crate::{
    apis::trigger::{TriggerAction, TriggerError, TriggerManager},
    config::Config,
};
use anyhow::Result;
use layer_climb::prelude::*;
use tokio::{runtime::Runtime, sync::mpsc};

#[derive(Clone)]
pub struct CoreTriggerManager {
    pub config: Config,
    pub runtime: Arc<Runtime>,
    pub kill_receiver: Arc<std::sync::Mutex<Option<tokio::sync::broadcast::Receiver<()>>>>,
}

impl CoreTriggerManager {
    pub fn new(
        config: Config,
        runtime: Arc<Runtime>,
        kill_receiver: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<Self, TriggerError> {
        Ok(Self {
            config,
            runtime,
            kill_receiver: Arc::new(std::sync::Mutex::new(Some(kill_receiver))),
        })
    }

    async fn start_producer(
        &self,
        _query_client: QueryClient,
        _action_sender: mpsc::UnboundedSender<TriggerAction>,
    ) -> Result<(), TriggerError> {
        std::future::pending::<()>().await;

        Ok(())
    }
}

impl TriggerManager for CoreTriggerManager {
    fn start(&self) -> Result<mpsc::UnboundedReceiver<TriggerAction>, TriggerError> {
        let chain_config = self
            .config
            .chain_config()
            .map_err(TriggerError::QueryClient)?;

        // The trigger manager should be free to quickly fire off triggers
        // so that it can continue to monitor the chain
        // if there are any backpressure issues, it should be dealt with on the dispatcher side
        // e.g. holding a limited local queue of triggers to be processed, after being recieved from the channel
        let (tx, rx) = mpsc::unbounded_channel();

        let mut kill_receiver = self.kill_receiver.lock().unwrap().take().unwrap();

        let _self = self.clone();

        self.runtime.spawn(async move {
            // get a chain query client
            let query_client = QueryClient::new(chain_config)
                .await
                .map_err(TriggerError::QueryClient)
                .unwrap();

            tracing::info!(
                "Trigger Manager started on {}",
                query_client.chain_config.chain_id
            );

            tokio::select! {
                _ = kill_receiver.recv() => {
                    tracing::info!("Trigger Manager shutting down");
                },
                _ = _self.start_producer(query_client, tx) => {
                }
            }
        });

        Ok(rx)
    }

    fn add_trigger(&self, _trigger: crate::apis::trigger::TriggerData) -> Result<(), TriggerError> {
        todo!()
    }

    fn remove_trigger(
        &self,
        _service_id: crate::apis::ID,
        _workflow_id: crate::apis::ID,
    ) -> Result<(), TriggerError> {
        todo!()
    }

    fn remove_service(&self, _service_id: crate::apis::ID) -> Result<(), TriggerError> {
        todo!()
    }

    fn list_triggers(
        &self,
        _service_id: crate::apis::ID,
    ) -> Result<Vec<crate::apis::trigger::TriggerData>, TriggerError> {
        todo!()
    }
}
