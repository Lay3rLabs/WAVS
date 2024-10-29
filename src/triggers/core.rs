use crate::apis::trigger::{TriggerError, TriggerManager};
use anyhow::Result;
use layer_climb::prelude::*;

pub struct CoreTriggerManager {
    pub query_client: QueryClient,
}

impl CoreTriggerManager {
    pub async fn new(chain_config: ChainConfig) -> Result<Self, TriggerError> {
        // get a chain query client
        let query_client = QueryClient::new(chain_config)
            .await
            .map_err(TriggerError::QueryClient)?;

        tracing::info!(
            "Trigger Manager created on chain: {}",
            query_client.chain_config.chain_id
        );

        Ok(Self { query_client })
    }
}

impl TriggerManager for CoreTriggerManager {
    fn receiver(&self) -> tokio::sync::mpsc::Receiver<crate::apis::trigger::TriggerAction> {
        todo!()
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
