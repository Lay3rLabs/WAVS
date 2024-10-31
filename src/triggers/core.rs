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
    pub all_trigger_data: Arc<RwLock<BTreeMap<LookupId, TriggerData>>>,
    /// lookup id by task queue address
    pub triggers_by_task_queue: Arc<RwLock<HashMap<Address, LookupId>>>,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow: Arc<RwLock<BTreeMap<ID, BTreeMap<ID, LookupId>>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
}

impl LookupMaps {
    pub fn new() -> Self {
        Self {
            all_trigger_data: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            triggers_by_task_queue: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_service_workflow: Arc::new(RwLock::new(BTreeMap::new())),
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
            .map_err(TriggerError::Climb)?;

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
                            let resp: Result<task_queue::TaskResponse> = query_client
                                .contract_smart(
                                    &contract_address,
                                    &task_queue::QueryMsg::Custom(
                                        task_queue::CustomQueryMsg::Task { id: task_id },
                                    ),
                                )
                                .await;

                            match resp {
                                Ok(resp) => {
                                    let result = TriggerResult::Queue {
                                        task_id,
                                        payload: serde_json::to_vec(&resp.payload).unwrap(),
                                    };

                                    let ids = {
                                        let triggers_by_task_queue_lock =
                                            lookup_maps.triggers_by_task_queue.read().unwrap();
                                        let all_trigger_data_lock =
                                            lookup_maps.all_trigger_data.read().unwrap();

                                        triggers_by_task_queue_lock
                                            .get(&contract_address)
                                            .ok_or_else(|| {
                                                TriggerError::NoSuchTaskQueueTrigger(
                                                    contract_address.clone(),
                                                )
                                            })
                                            .and_then(|lookup_id| {
                                                all_trigger_data_lock
                                                    .get(lookup_id)
                                                    .ok_or(TriggerError::NoSuchTriggerData(
                                                        *lookup_id,
                                                    ))
                                                    .map(|service_workflow_ids| {
                                                        (
                                                            service_workflow_ids.service_id.clone(),
                                                            service_workflow_ids
                                                                .workflow_id
                                                                .clone(),
                                                        )
                                                    })
                                            })
                                    };

                                    match ids {
                                        Ok((service_id, workflow_id)) => {
                                            action_sender
                                                .send(TriggerAction {
                                                    service_id,
                                                    workflow_id,
                                                    result,
                                                })
                                                .await
                                                .unwrap();
                                        }
                                        Err(err) => {
                                            tracing::error!("error finding task: {:?}", err);
                                        }
                                    }
                                }
                                Err(err) => {
                                    tracing::error!("error querying task queue: {:?}", err);
                                }
                            }
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
                    .triggers_by_task_queue
                    .write()
                    .unwrap()
                    .insert(addr, lookup_id);
            }
        }

        // adding it to our lookups is the same, regardless of type
        self.lookup_maps
            .triggers_by_service_workflow
            .write()
            .unwrap()
            .entry(data.service_id.clone())
            .or_default()
            .insert(data.workflow_id.clone(), lookup_id);

        self.lookup_maps
            .all_trigger_data
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
        let mut service_lock = self
            .lookup_maps
            .triggers_by_service_workflow
            .write()
            .unwrap();

        let workflow_map = service_lock
            .get_mut(&service_id)
            .ok_or_else(|| TriggerError::NoSuchService(service_id.clone()))?;

        // first remove it from services
        let lookup_id = workflow_map
            .remove(&workflow_id)
            .ok_or(TriggerError::NoSuchWorkflow(service_id, workflow_id))?;

        remove_trigger_data(
            &mut self.lookup_maps.all_trigger_data.write().unwrap(),
            &mut self.lookup_maps.triggers_by_task_queue.write().unwrap(),
            &self.chain_config,
            lookup_id,
        )?;

        Ok(())
    }

    fn remove_service(&self, service_id: crate::apis::ID) -> Result<(), TriggerError> {
        let mut all_trigger_data_lock = self.lookup_maps.all_trigger_data.write().unwrap();
        let mut triggers_by_task_queue_lock =
            self.lookup_maps.triggers_by_task_queue.write().unwrap();
        let mut triggers_by_service_workflow_lock = self
            .lookup_maps
            .triggers_by_service_workflow
            .write()
            .unwrap();

        let workflow_map = triggers_by_service_workflow_lock
            .get(&service_id)
            .ok_or_else(|| TriggerError::NoSuchService(service_id.clone()))?;

        for lookup_id in workflow_map.values() {
            remove_trigger_data(
                &mut all_trigger_data_lock,
                &mut triggers_by_task_queue_lock,
                &self.chain_config,
                *lookup_id,
            )?;
        }

        // 3. remove from service_workflow_lookup_map
        triggers_by_service_workflow_lock.remove(&service_id);

        Ok(())
    }

    fn list_triggers(&self, service_id: crate::apis::ID) -> Result<Vec<TriggerData>, TriggerError> {
        let mut triggers = Vec::new();

        let triggers_by_service_workflow_lock = self
            .lookup_maps
            .triggers_by_service_workflow
            .read()
            .unwrap();
        let all_trigger_data_lock = self.lookup_maps.all_trigger_data.read().unwrap();

        let workflow_map = triggers_by_service_workflow_lock
            .get(&service_id)
            .ok_or(TriggerError::NoSuchService(service_id))?;

        for lookup_id in workflow_map.values() {
            let trigger_data = all_trigger_data_lock
                .get(lookup_id)
                .ok_or(TriggerError::NoSuchTriggerData(*lookup_id))?;
            triggers.push(trigger_data.clone());
        }

        Ok(triggers)
    }
}

fn remove_trigger_data(
    all_trigger_data: &mut BTreeMap<usize, TriggerData>,
    triggers_by_task_queue: &mut HashMap<Address, LookupId>,
    chain_config: &ChainConfig,
    lookup_id: LookupId,
) -> Result<(), TriggerError> {
    // 1. remove from triggers
    let trigger_data = all_trigger_data
        .remove(&lookup_id)
        .ok_or(TriggerError::NoSuchTriggerData(lookup_id))?;

    // 2. remove from task_queue_lookup_map
    match &trigger_data.trigger {
        Trigger::Queue {
            task_queue_addr,
            poll_interval: _,
        } => {
            let addr = chain_config
                .parse_address(task_queue_addr)
                .map_err(TriggerError::Climb)?;
            triggers_by_task_queue
                .remove(&addr)
                .ok_or(TriggerError::NoSuchTaskQueueTrigger(addr))?;
        }
    }

    Ok(())
}
