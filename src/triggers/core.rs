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

#[cfg(test)]
mod tests {
    use crate::{
        apis::{
            trigger::{TriggerData, TriggerManager},
            Trigger, ID,
        },
        config::{Config, WasmaticChainConfig},
    };

    use super::CoreTriggerManager;

    #[test]
    fn core_trigger_lookups() {
        let config = Config {
            chain: "test".to_string(),
            chains: vec![(
                "test".to_string(),
                WasmaticChainConfig {
                    chain_id: "slay3r-local".parse().unwrap(),
                    rpc_endpoint: "http://localhost:26657".to_string(),
                    grpc_endpoint: "http://localhost:9090".to_string(),
                    gas_price: 0.025,
                    gas_denom: "uslay".to_string(),
                    bech32_prefix: "layer".to_string(),
                    faucet_endpoint: None,
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let manager = CoreTriggerManager::new(&config).unwrap();

        let service_id_1 = ID::new("service-1").unwrap();
        let workflow_id_1 = ID::new("workflow-1").unwrap();

        let service_id_2 = ID::new("service-2").unwrap();
        let workflow_id_2 = ID::new("workflow-2").unwrap();

        let task_queue_addr_1_1 = "layer13jwzcq8m4k4tyz6dwvqtnww0ds9vwptph0lnqm".to_string();
        let task_queue_addr_1_2 = "layer1aktndkmndlxd60ep7g58vc3wxkpqd0hn7ngj2w".to_string();
        let task_queue_addr_2_1 = "layer18aa3r27pk2vtsvqfwj045vheyjsu3hv6e3h6qw".to_string();
        let task_queue_addr_2_2 = "layer1jvuf2fye4sr09sn4042a5c30zf22u8ar60apyp".to_string();

        let trigger_1_1 = make_trigger(&service_id_1, &workflow_id_1, &task_queue_addr_1_1);
        let trigger_1_2 = make_trigger(&service_id_1, &workflow_id_2, &task_queue_addr_1_2);
        let trigger_2_1 = make_trigger(&service_id_2, &workflow_id_1, &task_queue_addr_2_1);
        let trigger_2_2 = make_trigger(&service_id_2, &workflow_id_2, &task_queue_addr_2_2);

        manager.add_trigger(trigger_1_1).unwrap();
        manager.add_trigger(trigger_1_2).unwrap();
        manager.add_trigger(trigger_2_1).unwrap();
        manager.add_trigger(trigger_2_2).unwrap();

        let triggers_service_1 = manager.list_triggers(service_id_1.clone()).unwrap();

        assert_eq!(triggers_service_1.len(), 2);
        assert_eq!(triggers_service_1[0].service_id, service_id_1);
        assert_eq!(triggers_service_1[0].workflow_id, workflow_id_1);
        assert_eq!(
            get_trigger_addr(&triggers_service_1[0].trigger),
            &task_queue_addr_1_1
        );
        assert_eq!(triggers_service_1[1].service_id, service_id_1);
        assert_eq!(triggers_service_1[1].workflow_id, workflow_id_2);
        assert_eq!(
            get_trigger_addr(&triggers_service_1[1].trigger),
            &task_queue_addr_1_2
        );

        let triggers_service_2 = manager.list_triggers(service_id_2.clone()).unwrap();

        assert_eq!(triggers_service_2.len(), 2);
        assert_eq!(triggers_service_2[0].service_id, service_id_2);
        assert_eq!(triggers_service_2[0].workflow_id, workflow_id_1);
        assert_eq!(
            get_trigger_addr(&triggers_service_2[0].trigger),
            &task_queue_addr_2_1
        );
        assert_eq!(triggers_service_2[1].service_id, service_id_2);
        assert_eq!(triggers_service_2[1].workflow_id, workflow_id_2);
        assert_eq!(
            get_trigger_addr(&triggers_service_2[1].trigger),
            &task_queue_addr_2_2
        );

        manager
            .remove_trigger(service_id_1.clone(), workflow_id_1)
            .unwrap();
        let triggers_service_1 = manager.list_triggers(service_id_1.clone()).unwrap();
        let triggers_service_2 = manager.list_triggers(service_id_2.clone()).unwrap();
        assert_eq!(triggers_service_1.len(), 1);
        assert_eq!(triggers_service_2.len(), 2);

        manager.remove_service(service_id_2.clone()).unwrap();
        let triggers_service_1 = manager.list_triggers(service_id_1.clone()).unwrap();
        let _triggers_service_2_err = manager.list_triggers(service_id_2.clone()).unwrap_err();
        assert_eq!(triggers_service_1.len(), 1);

        fn make_trigger(service_id: &ID, workflow_id: &ID, task_queue_addr: &str) -> TriggerData {
            TriggerData {
                service_id: service_id.clone(),
                workflow_id: workflow_id.clone(),
                trigger: Trigger::Queue {
                    task_queue_addr: task_queue_addr.to_string(),
                    poll_interval: 5,
                },
            }
        }

        fn get_trigger_addr(trigger: &Trigger) -> &str {
            match trigger {
                Trigger::Queue {
                    task_queue_addr,
                    poll_interval: _,
                } => task_queue_addr,
            }
        }
    }
}
