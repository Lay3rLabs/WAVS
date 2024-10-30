use std::collections::{HashMap, HashSet};

use crate::{
    apis::{
        trigger::{TriggerAction, TriggerError, TriggerManager, TriggerResult},
        ID,
    },
    config::Config,
    context::AppContext,
};
use anyhow::Result;
use futures::StreamExt;
use lavs_apis::{events::task_queue_events::TaskCreatedEvent, id::TaskId, tasks as task_queue};
use layer_climb::prelude::*;
use tokio::sync::mpsc;

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

    async fn start_watcher(
        &self,
        action_sender: mpsc::Sender<TriggerAction>,
    ) -> Result<(), TriggerError> {
        let query_client = QueryClient::new(self.chain_config.clone())
            .await
            .map_err(TriggerError::QueryClient)
            .unwrap();

        tracing::info!(
            "Trigger Manager started on {}",
            query_client.chain_config.chain_id
        );

        let event_stream = Box::pin(query_client.clone().stream_block_events(None))
            .await
            .map_err(TriggerError::EventStream)?;

        event_stream
            .for_each(|block_events| {
                let query_client = query_client.clone();
                let action_sender = action_sender.clone();
                async move {
                    let mut task_created_events: HashMap<Address, HashSet<TaskId>> = HashMap::new();

                    match block_events {
                        Ok(block_events) => {
                            let events = CosmosTxEvents::from(block_events.events);

                            for event in events.events_iter().map(cosmwasm_std::Event::from) {
                                if let Ok(task_event) = TaskCreatedEvent::try_from(&event) {
                                    let contract_address =
                                        event.attributes.iter().find_map(|attr| {
                                            if attr.key == "_contract_address" {
                                                query_client
                                                    .chain_config
                                                    .parse_address(&attr.value)
                                                    .ok()
                                            } else {
                                                None
                                            }
                                        });

                                    if let Some(contract_address) = contract_address {
                                        task_created_events
                                            .entry(contract_address)
                                            .or_default()
                                            .insert(task_event.task_id);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!("Error: {:?}", err);
                        }
                    }

                    for (contract_address, task_ids) in task_created_events {
                        for task_id in task_ids {
                            let resp: task_queue::TaskResponse = query_client
                                .contract_smart(
                                    &contract_address,
                                    &task_queue::QueryMsg::Custom(
                                        task_queue::CustomQueryMsg::Task { id: task_id },
                                    ),
                                )
                                .await
                                .map_err(TriggerError::QueryClient)
                                .unwrap();

                            let result = TriggerResult::Queue {
                                task_id,
                                payload: serde_json::to_vec(&resp.payload).unwrap(),
                            };

                            tracing::info!("Triggering action for task_id: {}", task_id);

                            action_sender
                                .send(TriggerAction {
                                    service_id: ID::new("todo-service").unwrap(),
                                    workflow_id: ID::new("todo-workflow").unwrap(),
                                    result,
                                })
                                .await
                                .unwrap();
                        }
                    }
                }
            })
            .await;

        tracing::info!("Trigger Manager watcher finished");

        Ok(())
    }
}

impl TriggerManager for CoreTriggerManager {
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        // The trigger manager should be free to quickly fire off triggers
        // so that it can continue to monitor the chain
        // it's up to the dispatcher to alleviate the backpressure
        let (action_sender, action_receiver) = mpsc::channel(self.channel_bound);

        ctx.rt.clone().spawn({
            let _self = self.clone();
            let mut kill_receiver = ctx.get_kill_receiver();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::info!("Trigger Manager shutting down");
                    },
                    _ = _self.start_watcher(action_sender) => {
                    }
                }
            }
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
