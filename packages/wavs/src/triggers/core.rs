use crate::{
    apis::{
        trigger::{
            Trigger, TriggerAction, TriggerConfig, TriggerData, TriggerError, TriggerManager,
        },
        EthHelloWorldTaskRlp, ServiceID, WorkflowID,
    },
    config::Config,
    AppContext,
};
use alloy::{
    providers::Provider,
    rpc::types::{Filter, Log},
    sol_types::SolEvent,
};
use alloy_rlp::Encodable;
use anyhow::Result;
use futures::{Stream, StreamExt};
use lavs_apis::{events::task_queue_events::TaskCreatedEvent, id::TaskId, tasks as task_queue};
use layer_climb::prelude::*;
use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
};

#[derive(Clone)]
pub struct CoreTriggerManager {
    // key: ChainID
    pub cosmos_chain_configs: HashMap<String, layer_climb::prelude::ChainConfig>,
    // key: ChainID
    pub ethereum_chain_configs: HashMap<String, EthClientConfig>,
    pub channel_bound: usize,
    lookup_maps: Arc<LookupMaps>,
}

pub enum BlockTriggers {
    EthereumLog {
        log: Log,
    },
    Layer {
        // TODO: this feels very inefficient but works for now (i.e. make it contract based?).
        triggers: HashMap<Address, HashMap<TaskId, QueryClient>>,
    },
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
        let enabled_configs = match config.enabled_chain_configs() {
            Ok(enabled) => enabled,
            Err(e) => return Err(TriggerError::ChainConfig(e)),
        };

        let mut ethereum_chain_configs = HashMap::new();
        let mut cosmos_chain_configs = HashMap::new();

        for (chain_id, eth_cfg) in enabled_configs.eth.iter() {
            tracing::debug!(
                "Ethereum chain config: {} -> {}",
                chain_id,
                eth_cfg.ws_endpoint
            );
            let ec: EthClientConfig = eth_cfg.clone().into();
            ethereum_chain_configs.insert(eth_cfg.chain_id.to_string(), ec);
        }

        for (chain_id, cosmos_cfg) in enabled_configs.cosmos.iter() {
            tracing::debug!(
                "Cosmos chain config: {} -> {}",
                chain_id,
                cosmos_cfg.rpc_endpoint
            );
            let cc: layer_climb::prelude::ChainConfig = cosmos_cfg.clone().into();
            cosmos_chain_configs.insert(chain_id.to_string(), cc);
        }

        Ok(Self {
            cosmos_chain_configs,
            ethereum_chain_configs,
            channel_bound: 100, // TODO: get from config
            lookup_maps: Arc::new(LookupMaps::new()),
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    async fn start_watcher(
        &self,
        action_sender: mpsc::Sender<TriggerAction>,
    ) -> Result<(), TriggerError> {
        let mut streams: Vec<Pin<Box<dyn Stream<Item = Result<BlockTriggers>> + Send>>> =
            Vec::new();

        // Create clients for each Cosmos chain
        for (_chain_id, chain_config) in self.cosmos_chain_configs.iter() {
            if let Ok(query_client) = QueryClient::new(chain_config.clone()).await {
                tracing::debug!(
                    "Trigger Manager for Cosmos chain started on {} at {}",
                    query_client.chain_config.chain_id,
                    query_client.chain_config.rpc_endpoint,
                );

                let chain_config = query_client.chain_config.clone();
                let query_client_clone = query_client.clone();
                let event_stream: Pin<Box<dyn Stream<Item = Result<BlockTriggers>> + Send>> =
                    Box::pin(
                        query_client
                            .stream_block_events(None)
                            .await
                            .map_err(TriggerError::Climb)?
                            .map(move |block_events| {
                                let mut task_created_events: HashMap<
                                    Address,
                                    HashMap<TaskId, QueryClient>,
                                > = HashMap::new();

                                match block_events {
                                    Ok(block_events) => {
                                        let events = CosmosTxEvents::from(block_events.events);
                                        for event in
                                            events.events_iter().map(cosmwasm_std::Event::from)
                                        {
                                            if let Ok(task_event) =
                                                TaskCreatedEvent::try_from(&event)
                                            {
                                                if let Some(contract_address) =
                                                    event.attributes.iter().find_map(|attr| {
                                                        if attr.key == "_contract_address" {
                                                            chain_config
                                                                .parse_address(&attr.value)
                                                                .ok()
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                {
                                                    task_created_events
                                                        .entry(contract_address.clone())
                                                        .or_default()
                                                        .insert(
                                                            task_event.task_id,
                                                            query_client_clone.clone(),
                                                        );
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
        }

        // let mut streams = futures::stream::select_all(streams);

        // Create clients for each Ethereum chain
        for (_chain_id, chain_config) in self.ethereum_chain_configs.iter() {
            let config = chain_config.clone();
            let action_sender = action_sender.clone();
            let self_lookup_maps = Arc::clone(&self.lookup_maps);
            let stream = tokio::spawn(async move {
                loop {
                    match EthClientBuilder::new(config.clone()).build_query().await {
                        Ok(query_client) => {
                            tracing::info!(
                                "Trigger Manager for Ethereum chain started on {}",
                                config.ws_endpoint
                            );

                            let filter =
                                Filter::new().event_signature(NewTaskCreated::SIGNATURE_HASH);
                            tracing::debug!(
                                "Created filter with signature: {:?}",
                                NewTaskCreated::SIGNATURE_HASH
                            );

                            match query_client.ws_provider.subscribe_logs(&filter).await {
                                Ok(subscription) => {
                                    tracing::info!("Successfully subscribed to logs with filter");
                                    let stream = subscription.into_stream();

                                    let mut stream =
                                        stream.map(|log| -> Result<BlockTriggers, TriggerError> {
                                            tracing::debug!("Received log: {:?}", log);
                                            Ok(BlockTriggers::EthereumLog { log })
                                        });

                                    while let Some(result) = stream.next().await {
                                        match result {
                                            Ok(trigger) => {
                                                if let BlockTriggers::EthereumLog { log } = trigger
                                                {
                                                    if let Ok(event) = log
                                                        .log_decode::<NewTaskCreated>()
                                                        .map(|log| log.inner.data)
                                                    {
                                                        let contract_address = Address::Eth(
                                                            AddrEth::new(log.address().into()),
                                                        );
                                                        let task_id =
                                                            TaskId::new(event.taskIndex.into());

                                                        let mut payload = Vec::new();
                                                        EthHelloWorldTaskRlp {
                                                            name: event.task.name,
                                                            created_block: event
                                                                .task
                                                                .taskCreatedBlock,
                                                        }
                                                        .encode(&mut payload);

                                                        // Look up trigger data
                                                        let lookup_id = {
                                                            let triggers_by_task_queue_lock =
                                                                self_lookup_maps
                                                                    .triggers_by_task_queue
                                                                    .read()
                                                                    .unwrap();

                                                            match triggers_by_task_queue_lock
                                                                .get(&contract_address)
                                                            {
                                                                Some(lookup_id) => *lookup_id,
                                                                None => {
                                                                    tracing::error!("No trigger found for task queue: {:?}", contract_address);
                                                                    continue;
                                                                }
                                                            }
                                                        };

                                                        let trigger = {
                                                            let all_trigger_data_lock =
                                                                self_lookup_maps
                                                                    .all_trigger_data
                                                                    .read()
                                                                    .unwrap();

                                                            all_trigger_data_lock
                                                                .get(&lookup_id)
                                                                .ok_or(
                                                                    TriggerError::NoSuchTriggerData(
                                                                        lookup_id,
                                                                    ),
                                                                )
                                                                .cloned()
                                                        };

                                                        match trigger {
                                                            Ok(trigger) => {
                                                                if let Err(e) = action_sender
                                                                    .send(TriggerAction {
                                                                        config: trigger,
                                                                        data: TriggerData::Queue {
                                                                            task_id,
                                                                            payload,
                                                                        },
                                                                    })
                                                                    .await
                                                                {
                                                                    // If send fails, likely means receiver was dropped - exit loop
                                                                    tracing::debug!("Action sender closed, exiting: {}", e);
                                                                    return;
                                                                }
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("Error getting trigger data: {:?}", e);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!("Error processing event: {:?}", e);
                                                break;
                                            }
                                        }
                                    }
                                    // Stream ended normally, exit the loop
                                    tracing::debug!("WebSocket stream closed, exiting");
                                    return;
                                }
                                Err(e) => {
                                    tracing::error!("Failed to subscribe to logs: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to build query client: {:?}", e);
                        }
                    }

                    // Wait before reconnecting
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
        }



        // Keep the watcher running
        futures::future::pending::<()>().await;

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
    }

    async fn handle_trigger(
        &self,
        action_sender: &mpsc::Sender<TriggerAction>,
        contract_address: &Address,
        task_id: TaskId,
        payload: Vec<u8>,
    ) {
        // Add debug logging
        tracing::debug!(
            "Handling trigger for contract: {:?}, task_id: {:?}",
            contract_address,
            task_id
        );

        let lookup_id = {
            let triggers_by_task_queue_lock =
                self.lookup_maps.triggers_by_task_queue.read().unwrap();

            match triggers_by_task_queue_lock.get(contract_address) {
                Some(lookup_id) => *lookup_id,
                None => {
                    tracing::error!("No trigger found for task queue: {:?}", contract_address);
                    return;
                }
            }
        };

        // Add more logging for trigger lookup
        tracing::debug!("Found lookup_id: {} for contract", lookup_id);

        let trigger = {
            let all_trigger_data_lock = self.lookup_maps.all_trigger_data.read().unwrap();

            all_trigger_data_lock
                .get(&lookup_id)
                .ok_or(TriggerError::NoSuchTriggerData(lookup_id))
                .cloned()
        };

        match trigger {
            Ok(trigger) => {
                tracing::debug!("Sending trigger action for task_id: {:?}", task_id);
                if let Err(e) = action_sender
                    .send(TriggerAction {
                        config: trigger,
                        data: TriggerData::Queue { task_id, payload },
                    })
                    .await
                {
                    tracing::error!("Failed to send trigger action: {:?}", e);
                }
            }
            Err(err) => {
                tracing::error!("Error finding trigger data: {:?}", err);
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
            Trigger::EthQueue { task_queue_addr } => {
                self.lookup_maps
                    .triggers_by_task_queue
                    .write()
                    .unwrap()
                    .insert(task_queue_addr.clone(), lookup_id);
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
        Trigger::EthQueue { task_queue_addr } => {
            triggers_by_task_queue.remove(task_queue_addr).ok_or(
                TriggerError::NoSuchTaskQueueTrigger(task_queue_addr.clone()),
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
            enabled_cosmos: vec!["test-cosmos".to_string()],
            enabled_ethereum: vec!["test-eth".to_string()],
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
                        rpc_endpoint: "http://localhost:26657".to_string(),
                        grpc_endpoint: "http://localhost:9090".to_string(),
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
            TriggerConfig::eth_queue(&service_id_1, &workflow_id_1, task_queue_addr_1_1.clone())
                .unwrap();
        let trigger_1_2 =
            TriggerConfig::eth_queue(&service_id_1, &workflow_id_2, task_queue_addr_1_2.clone())
                .unwrap();
        let trigger_2_1 =
            TriggerConfig::eth_queue(&service_id_2, &workflow_id_1, task_queue_addr_2_1.clone())
                .unwrap();
        let trigger_2_2 =
            TriggerConfig::eth_queue(&service_id_2, &workflow_id_2, task_queue_addr_2_2.clone())
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
                Trigger::EthQueue { task_queue_addr } => task_queue_addr,
            }
        }
    }
}
