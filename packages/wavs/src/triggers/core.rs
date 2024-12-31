use crate::{
    apis::{
        trigger::{
            Trigger, TriggerAction, TriggerConfig, TriggerData, TriggerError, TriggerManager,
        },
        ServiceID, WorkflowID,
    },
    config::Config,
    AppContext,
};
use alloy::{
    providers::Provider,
    rpc::types::{Filter, Log},
    sol_types::SolEvent,
};
use anyhow::Result;
use futures::{Stream, StreamExt};
use lavs_apis::{events::task_queue_events::TaskCreatedEvent, id::TaskId, tasks as task_queue};
use layer_climb::prelude::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    eth_client::{EthClientBuilder, EthClientConfig},
    layer_contract_client::{
        layer_trigger::LayerTrigger::{self, NewTrigger},
        TriggerId,
    },
};

#[derive(Clone)]
pub struct CoreTriggerManager {
    pub cosmos_chain_config: Option<layer_climb::prelude::ChainConfig>,
    pub chain_config: Option<EthClientConfig>,
    pub channel_bound: usize,
    lookup_maps: Arc<LookupMaps>,
}

struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    pub all_trigger_data: Arc<RwLock<BTreeMap<LookupId, TriggerConfig>>>,
    /// lookup id by task queue address
    pub triggers_by_task_queue: Arc<RwLock<HashMap<Address, LookupId>>>,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow:
        Arc<RwLock<BTreeMap<ServiceID, BTreeMap<WorkflowID, LookupId>>>>,
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
    #[instrument(level = "debug", fields(subsys = "TriggerManager"))]
    pub fn new(config: &Config) -> Result<Self, TriggerError> {
        let cosmos_chain_config = config
            .try_cosmos_chain_config()
            .map_err(TriggerError::Climb)?
            .map(|chain_config| chain_config.into());

        let chain_config = config
            .try_ethereum_chain_config()
            .map_err(TriggerError::Ethereum)?
            .map(|chain_config| chain_config.into());

        Ok(Self {
            cosmos_chain_config,
            chain_config,
            channel_bound: 100, // TODO: get from config
            lookup_maps: Arc::new(LookupMaps::new()),
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    async fn start_watcher(
        &self,
        action_sender: mpsc::Sender<TriggerAction>,
    ) -> Result<(), TriggerError> {
        // stream of streams, one for each chain
        let mut streams: Vec<Pin<Box<dyn Stream<Item = Result<BlockTriggers>> + Send>>> =
            Vec::new();

        enum BlockTriggers {
            EthereumLog {
                log: Log,
            },
            Layer {
                triggers: HashMap<Address, HashSet<TaskId>>,
            },
        }

        let cosmos_client = match self.cosmos_chain_config.clone() {
            Some(chain_config) => Some(
                QueryClient::new(chain_config, None)
                    .await
                    .map_err(TriggerError::Climb)?,
            ),
            None => None,
        };

        let ethereum_client = match self.chain_config.clone() {
            Some(chain_config) => {
                tracing::debug!(
                    "Ethereum client started on {}",
                    chain_config.ws_endpoint.as_ref().unwrap()
                );
                Some(
                    EthClientBuilder::new(chain_config)
                        .build_query()
                        .await
                        .map_err(TriggerError::Ethereum)?,
                )
            }
            None => None,
        };

        if let Some(query_client) = cosmos_client.clone() {
            tracing::debug!(
                "Trigger Manager for Cosmos chain started on {}",
                query_client.chain_config.chain_id
            );

            let chain_config = query_client.chain_config.clone();
            let event_stream = Box::pin(
                query_client
                    .stream_block_events(None)
                    .await
                    .map_err(TriggerError::Climb)?
                    .map(move |block_events| {
                        let mut task_created_events: HashMap<Address, HashSet<TaskId>> =
                            HashMap::new();

                        match block_events {
                            Ok(block_events) => {
                                let events = CosmosTxEvents::from(block_events.events);

                                for event in events.events_iter().map(cosmwasm_std::Event::from) {
                                    if let Ok(task_event) = TaskCreatedEvent::try_from(&event) {
                                        let contract_address =
                                            event.attributes.iter().find_map(|attr| {
                                                if attr.key == "_contract_address" {
                                                    chain_config.parse_address(&attr.value).ok()
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

                        Ok(BlockTriggers::Layer {
                            triggers: task_created_events,
                        })
                    }),
            );

            streams.push(event_stream);
        }

        if let Some(query_client) = ethereum_client.clone() {
            tracing::debug!("Trigger Manager for Ethereum chain started");

            // Start the event stream
            let filter = Filter::new().event_signature(NewTrigger::SIGNATURE_HASH);

            let stream = query_client
                .provider
                .subscribe_logs(&filter)
                .await
                .map_err(|e| TriggerError::Ethereum(e.into()))?
                .into_stream();

            let event_stream =
                Box::pin(stream.map(move |log| Ok(BlockTriggers::EthereumLog { log })));

            streams.push(event_stream);
        }

        // Multiplex all the stream of streams
        let mut streams = futures::stream::select_all(streams);

        while let Some(res) = streams.next().await {
            match res {
                Err(err) => {
                    tracing::error!("{:?}", err);
                }
                Ok(BlockTriggers::EthereumLog { log }) => {
                    if let Ok(log) = log.log_decode::<NewTrigger>() {
                        let service_id = log.data().serviceId.to_string();
                        let workflow_id = log.data().workflowId.to_string();
                        let trigger_id = log.data().triggerId;
                        match (ServiceID::new(&service_id), WorkflowID::new(&workflow_id)) {
                            (Ok(service_id), Ok(workflow_id)) => {
                                let trigger_id = TriggerId::new(trigger_id);

                                let contract = LayerTrigger::new(
                                    log.address(),
                                    ethereum_client.as_ref().unwrap().provider.clone(),
                                );

                                if let Ok(payload) = contract
                                    .getTrigger(*trigger_id)
                                    .call()
                                    .await
                                    .map(|resp| resp._0.data.to_vec())
                                {
                                    self.handle_trigger(
                                        &action_sender,
                                        &Address::Eth(AddrEth::new(log.address().into())),
                                        TriggerData::EthEvent {
                                            service_id,
                                            workflow_id,
                                            payload,
                                            trigger_id,
                                        },
                                    )
                                    .await;
                                }
                            }
                            _ => {
                                tracing::error!("error parsing service_id ({service_id}) or workflow_id ({workflow_id})");
                            }
                        }
                    }
                }
                Ok(BlockTriggers::Layer { triggers }) => {
                    for (contract_address, task_ids) in triggers {
                        for task_id in task_ids {
                            let resp: Result<task_queue::TaskResponse> = cosmos_client
                                .as_ref()
                                .unwrap() // safe - only way we got this is by having a client in the first place
                                .contract_smart(
                                    &contract_address,
                                    &task_queue::QueryMsg::Custom(
                                        task_queue::CustomQueryMsg::Task { id: task_id },
                                    ),
                                )
                                .await;

                            let payload = match resp {
                                Ok(resp) => {
                                    if !matches!(resp.status, task_queue::Status::Open {}) {
                                        tracing::debug!("task is not open: {:?}", resp);
                                        continue;
                                    }
                                    resp.payload
                                }
                                Err(err) => {
                                    tracing::error!("error querying task queue: {:?}", err);
                                    continue;
                                }
                            };

                            let payload = serde_json::to_vec(&payload)
                                .map_err(|e| TriggerError::ParseAvsPayload(e.into()))?;

                            self.handle_trigger(
                                &action_sender,
                                &contract_address,
                                TriggerData::Queue { task_id, payload },
                            )
                            .await;
                        }
                    }
                }
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
    }

    async fn handle_trigger(
        &self,
        action_sender: &mpsc::Sender<TriggerAction>,
        // for now all triggers are "task queues" of some sort
        // but this will eventually be more generic
        contract_address: &Address,
        data: TriggerData,
    ) {
        let lookup_id = match &data {
            TriggerData::Queue { .. } => {
                let triggers_by_task_queue_lock =
                    self.lookup_maps.triggers_by_task_queue.read().unwrap();

                match triggers_by_task_queue_lock.get(contract_address) {
                    Some(lookup_id) => *lookup_id,
                    None => {
                        tracing::debug!("not our task queue: {:?}", contract_address);

                        return;
                    }
                }
            }
            TriggerData::EthEvent {
                service_id,
                workflow_id,
                ..
            } => {
                let triggers_by_service_workflow_lock = self
                    .lookup_maps
                    .triggers_by_service_workflow
                    .read()
                    .unwrap();

                match triggers_by_service_workflow_lock
                    .get(service_id)
                    .and_then(|map| map.get(workflow_id))
                {
                    Some(lookup_id) => *lookup_id,
                    None => {
                        tracing::debug!("not our service/workflow: {}/{}", service_id, workflow_id);
                        return;
                    }
                }
            }
        };

        let trigger = {
            let all_trigger_data_lock = self.lookup_maps.all_trigger_data.read().unwrap();

            all_trigger_data_lock
                .get(&lookup_id)
                .ok_or(TriggerError::NoSuchTriggerData(lookup_id))
                .cloned()
        };

        match trigger {
            Ok(trigger) => {
                action_sender
                    .send(TriggerAction {
                        config: trigger,
                        data,
                    })
                    .await
                    .unwrap();
            }
            Err(err) => {
                tracing::error!("error finding task: {:?}", err);
            }
        }
    }
}

impl TriggerManager for CoreTriggerManager {
    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "TriggerManager"))]
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
                        tracing::debug!("Trigger Manager shutting down");
                    },
                    _ = _self.start_watcher(action_sender) => {
                    }
                }
            }
        });

        Ok(action_receiver)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn add_trigger(&self, data: TriggerConfig) -> Result<(), TriggerError> {
        // get the next lookup id
        let lookup_id = self
            .lookup_maps
            .lookup_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        match &data.trigger {
            Trigger::LayerQueue {
                task_queue_addr,
                poll_interval: _,
            } => {
                self.lookup_maps
                    .triggers_by_task_queue
                    .write()
                    .unwrap()
                    .insert(task_queue_addr.clone(), lookup_id);
            }
            Trigger::EthEvent { contract_address } => {
                self.lookup_maps
                    .triggers_by_task_queue
                    .write()
                    .unwrap()
                    .insert(contract_address.clone(), lookup_id);
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

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_trigger(
        &self,
        service_id: crate::apis::ServiceID,
        workflow_id: crate::apis::WorkflowID,
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
            lookup_id,
        )?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_service(&self, service_id: crate::apis::ServiceID) -> Result<(), TriggerError> {
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
                *lookup_id,
            )?;
        }

        // 3. remove from service_workflow_lookup_map
        triggers_by_service_workflow_lock.remove(&service_id);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn list_triggers(
        &self,
        service_id: crate::apis::ServiceID,
    ) -> Result<Vec<TriggerConfig>, TriggerError> {
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
    all_trigger_data: &mut BTreeMap<usize, TriggerConfig>,
    triggers_by_task_queue: &mut HashMap<Address, LookupId>,
    lookup_id: LookupId,
) -> Result<(), TriggerError> {
    // 1. remove from triggers
    let trigger_data = all_trigger_data
        .remove(&lookup_id)
        .ok_or(TriggerError::NoSuchTriggerData(lookup_id))?;

    // 2. remove from task_queue_lookup_map
    match &trigger_data.trigger {
        Trigger::LayerQueue {
            task_queue_addr,
            poll_interval: _,
        } => {
            triggers_by_task_queue.remove(task_queue_addr).ok_or(
                TriggerError::NoSuchTaskQueueTrigger(task_queue_addr.clone()),
            )?;
        }
        Trigger::EthEvent { contract_address } => {
            triggers_by_task_queue.remove(contract_address).ok_or(
                TriggerError::NoSuchTaskQueueTrigger(contract_address.clone()),
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        apis::{
            trigger::{Trigger, TriggerConfig, TriggerManager},
            ServiceID, WorkflowID,
        },
        config::{ChainConfigs, Config, CosmosChainConfig, EthereumChainConfig},
        test_utils::address::rand_address_eth,
    };

    use layer_climb::prelude::*;

    use super::CoreTriggerManager;

    #[test]
    fn core_trigger_lookups() {
        let config = Config {
            chain: Some("test".to_string()),
            chains: ChainConfigs {
                eth: [(
                    "test-eth".to_string(),
                    EthereumChainConfig {
                        chain_id: "eth-local".parse().unwrap(),
                        ws_endpoint: "ws://localhost:26657".to_string(),
                        http_endpoint: "http://localhost:26657".to_string(),
                        aggregator_endpoint: Some("http://localhost:8001".to_string()),
                        faucet_endpoint: None,
                        submission_mnemonic: None,
                    },
                )]
                .into_iter()
                .collect(),
                cosmos: [(
                    "test-cosmos".to_string(),
                    CosmosChainConfig {
                        chain_id: "layer-local".parse().unwrap(),
                        rpc_endpoint: Some("http://localhost:26657".to_string()),
                        grpc_endpoint: Some("http://localhost:9090".to_string()),
                        gas_price: 0.025,
                        gas_denom: "uslay".to_string(),
                        bech32_prefix: "layer".to_string(),
                        faucet_endpoint: None,
                        submission_mnemonic: None,
                    },
                )]
                .into_iter()
                .collect(),
            },
            ..Default::default()
        };

        let manager = CoreTriggerManager::new(&config).unwrap();

        let service_id_1 = ServiceID::new("service-1").unwrap();
        let workflow_id_1 = WorkflowID::new("workflow-1").unwrap();

        let service_id_2 = ServiceID::new("service-2").unwrap();
        let workflow_id_2 = WorkflowID::new("workflow-2").unwrap();

        let task_queue_addr_1_1 = rand_address_eth();
        let task_queue_addr_1_2 = rand_address_eth();
        let task_queue_addr_2_1 = rand_address_eth();
        let task_queue_addr_2_2 = rand_address_eth();

        let trigger_1_1 =
            TriggerConfig::eth_event(&service_id_1, &workflow_id_1, task_queue_addr_1_1.clone())
                .unwrap();
        let trigger_1_2 =
            TriggerConfig::eth_event(&service_id_1, &workflow_id_2, task_queue_addr_1_2.clone())
                .unwrap();
        let trigger_2_1 =
            TriggerConfig::eth_event(&service_id_2, &workflow_id_1, task_queue_addr_2_1.clone())
                .unwrap();
        let trigger_2_2 =
            TriggerConfig::eth_event(&service_id_2, &workflow_id_2, task_queue_addr_2_2.clone())
                .unwrap();

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

        fn get_trigger_addr(trigger: &Trigger) -> &Address {
            match trigger {
                Trigger::LayerQueue {
                    task_queue_addr,
                    poll_interval: _,
                } => task_queue_addr,
                Trigger::EthEvent { contract_address } => contract_address,
            }
        }
    }
}
