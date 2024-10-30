use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use crate::{
    apis::{
        trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager, TriggerResult},
        Trigger, ID,
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
    lookup_maps: Arc<LookupMaps>,
}

struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    pub triggers: Arc<RwLock<BTreeMap<LookupId, TriggerData>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
    /// lookup id by task queue address
    pub task_queue_lookup_map: Arc<RwLock<HashMap<Address, LookupId>>>,
    /// lookup id by service id -> workflow id
    pub service_workflow_lookup_map: Arc<RwLock<BTreeMap<ID, BTreeMap<ID, LookupId>>>>,
}

impl LookupMaps {
    pub fn new() -> Self {
        Self {
            triggers: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            task_queue_lookup_map: Arc::new(RwLock::new(HashMap::new())),
            service_workflow_lookup_map: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

type LookupId = usize;

impl CoreTriggerManager {
    #[allow(clippy::new_without_default)]
    pub fn new(config: &Config) -> Result<Self, TriggerError> {
        let chain_config = config.chain_config().map_err(TriggerError::Climb)?;

        Ok(Self {
            chain_config,
            channel_bound: 100, // TODO: get from config
            lookup_maps: Arc::new(LookupMaps::new()),
        })
    }

    async fn start_watcher(
        &self,
        action_sender: mpsc::Sender<TriggerAction>,
    ) -> Result<(), TriggerError> {
        let query_client = QueryClient::new(self.chain_config.clone())
            .await
            .map_err(TriggerError::Climb)
            .unwrap();

        tracing::info!(
            "Trigger Manager started on {}",
            query_client.chain_config.chain_id
        );

        let event_stream = Box::pin(query_client.clone().stream_block_events(None))
            .await
            .map_err(TriggerError::Climb)?;

        event_stream
            .for_each(|block_events| {
                let query_client = query_client.clone();
                let action_sender = action_sender.clone();
                let lookup_maps = self.lookup_maps.clone();
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
                                .map_err(TriggerError::Climb)
                                .unwrap();

                            let result = TriggerResult::Queue {
                                task_id,
                                payload: serde_json::to_vec(&resp.payload).unwrap(),
                            };

                            let (service_id, workflow_id) = {
                                let addr_lock = lookup_maps.task_queue_lookup_map.read().unwrap();
                                let triggers_lock = lookup_maps.triggers.read().unwrap();

                                let lookup_id = addr_lock.get(&contract_address).unwrap();
                                let service_workflow_ids = triggers_lock.get(lookup_id).unwrap();

                                (
                                    service_workflow_ids.service_id.clone(),
                                    service_workflow_ids.workflow_id.clone(),
                                )
                            };

                            action_sender
                                .send(TriggerAction {
                                    service_id,
                                    workflow_id,
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

    fn add_trigger(&self, data: TriggerData) -> Result<(), TriggerError> {
        // get the next lookup id
        let lookup_id = self
            .lookup_maps
            .lookup_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        match &data.trigger {
            Trigger::Queue {
                task_queue_addr,
                poll_interval: _,
            } => {
                // parse the address
                let addr = self
                    .chain_config
                    .parse_address(task_queue_addr)
                    .map_err(TriggerError::Climb)?;
                self.lookup_maps
                    .task_queue_lookup_map
                    .write()
                    .unwrap()
                    .insert(addr, lookup_id);
            }
        }

        // adding it to our lookups is the same, regardless of type
        self.lookup_maps
            .service_workflow_lookup_map
            .write()
            .unwrap()
            .entry(data.service_id.clone())
            .or_default()
            .insert(data.workflow_id.clone(), lookup_id);

        self.lookup_maps
            .triggers
            .write()
            .unwrap()
            .insert(lookup_id, data);
        Ok(())
    }

    fn remove_trigger(
        &self,
        service_id: crate::apis::ID,
        workflow_id: crate::apis::ID,
    ) -> Result<(), TriggerError> {
        // need to remove from:
        // 1. triggers
        // 2. (if task queue) task_queue_lookup_map
        // 3. service_workflow_lookup_map

        let mut service_lock = self
            .lookup_maps
            .service_workflow_lookup_map
            .write()
            .unwrap();
        let workflow_map = service_lock.get_mut(&service_id).unwrap();
        let lookup_id = *workflow_map.get(&workflow_id).unwrap();
        let mut triggers_lock = self.lookup_maps.triggers.write().unwrap();

        // 1. remove from triggers
        let trigger_data = triggers_lock.remove(&lookup_id).unwrap();

        // 2. remove from task_queue_lookup_map
        match &trigger_data.trigger {
            Trigger::Queue {
                task_queue_addr,
                poll_interval: _,
            } => {
                let addr = self
                    .chain_config
                    .parse_address(task_queue_addr)
                    .map_err(TriggerError::Climb)?;
                self.lookup_maps
                    .task_queue_lookup_map
                    .write()
                    .unwrap()
                    .remove(&addr);
            }
        }

        // 3. remove from service_workflow_lookup_map
        workflow_map.remove(&workflow_id);

        Ok(())
    }

    fn remove_service(&self, service_id: crate::apis::ID) -> Result<(), TriggerError> {
        // need to remove from:
        // 1. triggers
        // 2. (if task queue) task_queue_lookup_map
        // 3. service_workflow_lookup_map

        let mut service_lock = self
            .lookup_maps
            .service_workflow_lookup_map
            .write()
            .unwrap();

        for lookup_id in service_lock.get(&service_id).unwrap().values() {
            let mut triggers_lock = self.lookup_maps.triggers.write().unwrap();

            // 1. remove from triggers
            let trigger_data = triggers_lock.remove(lookup_id).unwrap();

            // 2. remove from task_queue
            match &trigger_data.trigger {
                Trigger::Queue {
                    task_queue_addr,
                    poll_interval: _,
                } => {
                    let addr = self
                        .chain_config
                        .parse_address(task_queue_addr)
                        .map_err(TriggerError::Climb)?;
                    self.lookup_maps
                        .task_queue_lookup_map
                        .write()
                        .unwrap()
                        .remove(&addr);
                }
            }
        }

        // 3. remove from service_workflow_lookup_map
        service_lock.remove(&service_id);

        Ok(())
    }

    fn list_triggers(&self, service_id: crate::apis::ID) -> Result<Vec<TriggerData>, TriggerError> {
        let mut triggers = Vec::new();

        let service_lock = self.lookup_maps.service_workflow_lookup_map.read().unwrap();
        let triggers_lock = self.lookup_maps.triggers.read().unwrap();

        let workflow_map = service_lock.get(&service_id).unwrap();
        for lookup_id in workflow_map.values() {
            triggers.push(triggers_lock.get(lookup_id).unwrap().clone());
        }

        Ok(triggers)
    }
}
