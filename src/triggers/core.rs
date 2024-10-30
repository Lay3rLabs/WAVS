use crate::{
    apis::trigger::{TriggerAction, TriggerError, TriggerManager},
    config::Config,
    context::AppContext,
};
use anyhow::Result;
use layer_climb::prelude::*;

#[derive(Clone)]
pub struct CoreTriggerManager {
    pub chain_config: ChainConfig,
    pub channel_bound: usize,
}

impl CoreTriggerManager {
    #[allow(clippy::new_without_default)]
    pub fn new(config: &Config) -> Result<Self, TriggerError> {
        let chain_config = config.chain_config().map_err(TriggerError::QueryClient)?;

        Ok(Self {
            chain_config,
            channel_bound: 100, // TODO: get from config
        })
    }
}

impl TriggerManager for CoreTriggerManager {
    fn start(
        &self,
        ctx: AppContext,
    ) -> Result<crossbeam_channel::Receiver<TriggerAction>, TriggerError> {
        // The trigger manager should be free to quickly fire off triggers
        // so that it can continue to monitor the chain
        // it's up to the dispatcher to alleviate the backpressure
        let (action_sender, action_receiver) = crossbeam_channel::bounded(self.channel_bound);

        let chain_config = self.chain_config.clone();

        ctx.rt.clone().spawn(async move {
            let query_client = QueryClient::new(chain_config)
                .await
                .map_err(TriggerError::QueryClient)
                .unwrap();

            let _action_sender = action_sender;

            tracing::info!(
                "Trigger Manager started on {}",
                query_client.chain_config.chain_id
            );

            // TODO: start watching
            std::future::pending::<()>().await;

            tracing::info!("Trigger Manager watcher finished");
        });

        Ok(action_receiver)
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
