use std::sync::Arc;

use crate::{
    apis::trigger::{TriggerAction, TriggerError, TriggerManager},
    config::Config,
};
use anyhow::Result;
use layer_climb::prelude::*;
use tokio::{runtime::Runtime, sync::mpsc};

pub struct CoreTriggerManager {
    pub config: Config,
    pub runtime: Arc<Runtime>,
}

impl CoreTriggerManager {
    pub fn new(config: Config, runtime: Arc<Runtime>) -> Result<Self, TriggerError> {
        Ok(Self { config, runtime })
    }
}

impl TriggerManager for CoreTriggerManager {
    fn start(&self) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        let chain_config = self
            .config
            .chain_config()
            .map_err(TriggerError::QueryClient)?;

        // get a chain query client
        let query_client = self.runtime.block_on(async move {
            QueryClient::new(chain_config)
                .await
                .map_err(TriggerError::QueryClient)
        })?;

        // TODO: the bounds here should be configurable
        // or maybe driven by same rough criteria as dispatcher component threads
        let (tx, rx) = mpsc::channel(100);

        self.runtime.spawn(async move {
            let _tx = tx;

            tracing::info!(
                "Trigger Manager started on {}",
                query_client.chain_config.chain_id
            );

            std::future::pending::<()>().await;
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
