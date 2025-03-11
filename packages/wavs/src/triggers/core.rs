use crate::{
    apis::trigger::{TriggerError, TriggerManager},
    config::Config,
    AppContext,
};
use alloy::{
    providers::Provider,
    rpc::types::{Filter, Log},
};
use anyhow::{Context, Result};
use futures::{Stream, StreamExt};
use layer_climb::prelude::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{config::AnyChainConfig, eth_client::EthClientBuilder};
use wavs_types::{
    ByteArray, ChainName, ServiceID, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowID,
};

#[derive(Clone)]
pub struct CoreTriggerManager {
    pub chain_configs: HashMap<ChainName, AnyChainConfig>,
    pub channel_bound: usize,
    lookup_maps: Arc<LookupMaps>,
}

#[allow(clippy::type_complexity)]
struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    pub trigger_configs: Arc<RwLock<BTreeMap<LookupId, TriggerConfig>>>,
    /// lookup id by (chain name, contract event address, event type)
    pub triggers_by_cosmos_contract_event:
        Arc<RwLock<HashMap<(ChainName, layer_climb::prelude::Address, String), HashSet<LookupId>>>>,
    /// lookup id by (chain id, contract event address, event hash)
    pub triggers_by_eth_contract_event: Arc<
        RwLock<HashMap<(ChainName, alloy::primitives::Address, ByteArray<32>), HashSet<LookupId>>>,
    >,
    /// lookup by chain_name -> n_blocks
    pub triggers_by_block_interval: Arc<RwLock<HashMap<ChainName, Vec<(u32, LookupId)>>>>,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow:
        Arc<RwLock<BTreeMap<ServiceID, BTreeMap<WorkflowID, LookupId>>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
}

impl LookupMaps {
    pub fn new() -> Self {
        Self {
            trigger_configs: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            triggers_by_cosmos_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_eth_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_block_interval: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_service_workflow: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

type LookupId = usize;

// *potential* triggers that we can react to
// this is just a local encapsulation, not a full trigger
// and is used to ultimately filter+map to a TriggerAction
enum StreamTriggers {
    Cosmos {
        chain_name: ChainName,
        // these are not filtered yet, just all the contract-based events
        contract_events: Vec<(Address, cosmwasm_std::Event)>,
        block_height: u64,
    },
    Ethereum {
        chain_name: ChainName,
        log: Log,
        block_height: u64,
    },
    // We need a separate stream for Ethereum block interval triggers
    EthereumBlock {
        chain_name: ChainName,
        block_height: u64,
    },
}

impl CoreTriggerManager {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "TriggerManager"))]
    pub fn new(config: &Config) -> Result<Self, TriggerError> {
        Ok(Self {
            chain_configs: config.active_trigger_chain_configs(),
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
        let mut streams: Vec<Pin<Box<dyn Stream<Item = Result<StreamTriggers>> + Send>>> =
            Vec::new();

        let mut cosmos_clients = HashMap::new();
        for (chain_name, chain_config) in self.chain_configs.clone() {
            if let AnyChainConfig::Cosmos(chain_config) = chain_config {
                let client = QueryClient::new(chain_config.into(), None)
                    .await
                    .map_err(TriggerError::Climb)?;

                cosmos_clients.insert(chain_name, client);
            }
        }

        let mut ethereum_clients = HashMap::new();
        for (chain_name, chain_config) in self.chain_configs.clone() {
            if let AnyChainConfig::Eth(chain_config) = chain_config {
                let client = EthClientBuilder::new(chain_config.to_client_config(None, None, None))
                    .build_query()
                    .await
                    .map_err(TriggerError::Ethereum)?;

                ethereum_clients.insert(chain_name, client);
            }
        }

        for (chain_name, query_client) in cosmos_clients.into_iter() {
            tracing::debug!("Trigger Manager for Cosmos chain {} started", chain_name);

            let chain_config = query_client.chain_config.clone();

            let event_stream = Box::pin(
                query_client
                    .stream_block_events(None)
                    .await
                    .map_err(TriggerError::Climb)?
                    .map(move |block_events| match block_events {
                        Ok(block_events) => {
                            let mut contract_events = Vec::new();
                            let events = CosmosTxEvents::from(block_events.events);

                            for event in events.events_iter() {
                                if event.ty().starts_with("wasm-") {
                                    let contract_address = event.attributes().find_map(|attr| {
                                        if attr.key() == "_contract_address" {
                                            chain_config.parse_address(attr.value()).ok()
                                        } else {
                                            None
                                        }
                                    });
                                    match contract_address {
                                        Some(contract_address) => {
                                            let mut event = cosmwasm_std::Event::from(event);
                                            event.ty =
                                                event.ty.strip_prefix("wasm-").unwrap().to_string();
                                            contract_events.push((contract_address, event));
                                        }
                                        None => {
                                            tracing::warn!(
                                                "Missing contract address in event: {:?}",
                                                event
                                            );
                                        }
                                    }
                                }
                            }

                            Ok(StreamTriggers::Cosmos {
                                chain_name: chain_name.clone(),
                                contract_events,
                                block_height: block_events.height,
                            })
                        }
                        Err(err) => Err(err),
                    }),
            );

            streams.push(event_stream);
        }

        for (chain_name, query_client) in ethereum_clients.iter() {
            tracing::debug!("Trigger Manager for Ethereum chain {} started", chain_name);

            // Start the event stream
            let filter = Filter::new();

            let stream = query_client
                .provider
                .subscribe_logs(&filter)
                .await
                .map_err(|e| TriggerError::Ethereum(e.into()))?
                .into_stream();

            let chain_name = chain_name.clone();

            let event_stream = Box::pin(stream.map(move |log| {
                Ok(StreamTriggers::Ethereum {
                    chain_name: chain_name.clone(),
                    block_height: log.block_number.context("couldn't get eth block height")?,
                    log,
                })
            }));

            streams.push(event_stream);
        }

        // Multiplex all the stream of streams
        let mut streams = futures::stream::select_all(streams);

        while let Some(res) = streams.next().await {
            let res = match res {
                Err(err) => {
                    tracing::error!("{:?}", err);
                    continue;
                }
                Ok(res) => res,
            };

            let mut trigger_actions = Vec::new();

            match res {
                StreamTriggers::Ethereum {
                    log,
                    chain_name,
                    block_height,
                } => {
                    if let Some(event_hash) = log.topic0() {
                        let contract_address = log.address();

                        let triggers_by_contract_event_lock = self
                            .lookup_maps
                            .triggers_by_eth_contract_event
                            .read()
                            .unwrap();

                        if let Some(lookup_ids) = triggers_by_contract_event_lock.get(&(
                            chain_name.clone(),
                            contract_address,
                            ByteArray::new(**event_hash),
                        )) {
                            let trigger_configs_lock =
                                self.lookup_maps.trigger_configs.read().unwrap();

                            for id in lookup_ids {
                                match trigger_configs_lock.get(id) {
                                    Some(trigger_config) => {
                                        trigger_actions.push(TriggerAction {
                                            data: TriggerData::EthContractEvent {
                                                contract_address,
                                                chain_name: chain_name.clone(),
                                                log: log.inner.data.clone(),
                                                block_height,
                                            },
                                            config: trigger_config.clone(),
                                        });
                                    }
                                    None => {
                                        tracing::error!(
                                            "Trigger config not found for lookup_id {}",
                                            id
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                StreamTriggers::Cosmos {
                    contract_events,
                    chain_name,
                    block_height,
                } => {
                    let triggers_by_contract_event_lock = self
                        .lookup_maps
                        .triggers_by_cosmos_contract_event
                        .read()
                        .unwrap();

                    let trigger_configs_lock = self.lookup_maps.trigger_configs.read().unwrap();

                    for (contract_address, event) in contract_events {
                        if let Some(lookup_ids) = triggers_by_contract_event_lock.get(&(
                            chain_name.clone(),
                            contract_address.clone(),
                            event.ty.clone(),
                        )) {
                            for id in lookup_ids {
                                match trigger_configs_lock.get(id) {
                                    Some(trigger_config) => {
                                        trigger_actions.push(TriggerAction {
                                            data: TriggerData::CosmosContractEvent {
                                                contract_address: contract_address.clone(),
                                                chain_name: chain_name.clone(),
                                                event: event.clone(),
                                                block_height,
                                            },
                                            config: trigger_config.clone(),
                                        });
                                    }
                                    None => {
                                        tracing::error!(
                                            "Trigger config not found for lookup_id {}",
                                            id
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Process block-based triggers
                    let mut triggers_by_block_interval_lock =
                        self.lookup_maps.triggers_by_block_interval.write().unwrap();
                    if let Some(countdowns) = triggers_by_block_interval_lock.get_mut(&chain_name) {
                        countdowns.iter_mut().for_each(|(countdown, lookup_id)| {
                            *countdown -= 1;

                            // If the countdown reaches zero, trigger the action
                            if *countdown == 0 {
                                let trigger_configs_lock =
                                    self.lookup_maps.trigger_configs.read().unwrap();
                                if let Some(trigger_config) = trigger_configs_lock.get(lookup_id) {
                                    if let Trigger::BlockInterval { n_blocks, .. } =
                                        &trigger_config.trigger
                                    {
                                        // Reset the countdown to `n_blocks`
                                        *countdown = *n_blocks;
                                        trigger_actions.push(TriggerAction {
                                            data: TriggerData::BlockInterval {
                                                chain_name: chain_name.clone(),
                                                block_height,
                                            },
                                            config: trigger_config.clone(),
                                        });
                                    }
                                }
                            }
                        });
                    }
                }
                StreamTriggers::EthereumBlock {
                    chain_name,
                    block_height,
                } => {
                    let triggers_by_contract_event_lock =
                        self.lookup_maps.triggers_by_block_interval.read().unwrap();

                    let trigger_configs_lock = self.lookup_maps.trigger_configs.read().unwrap();

                    unimplemented!();
                }
            }

            for action in trigger_actions {
                action_sender.send(action).await.unwrap();
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
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
    fn add_trigger(&self, config: TriggerConfig) -> Result<(), TriggerError> {
        // get the next lookup id
        let lookup_id = self
            .lookup_maps
            .lookup_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        match config.trigger.clone() {
            Trigger::EthContractEvent {
                address,
                chain_name,
                event_hash,
            } => {
                let mut lock = self
                    .lookup_maps
                    .triggers_by_eth_contract_event
                    .write()
                    .unwrap();
                let key = (chain_name.clone(), address, event_hash);

                lock.entry(key).or_default().insert(lookup_id);
            }
            Trigger::CosmosContractEvent {
                address,
                chain_name,
                event_type,
            } => {
                let mut lock = self
                    .lookup_maps
                    .triggers_by_cosmos_contract_event
                    .write()
                    .unwrap();
                let key = (chain_name.clone(), address.clone(), event_type.clone());

                lock.entry(key).or_default().insert(lookup_id);
            }
            Trigger::BlockInterval {
                chain_name,
                n_blocks,
            } => {
                let mut lock = self.lookup_maps.triggers_by_block_interval.write().unwrap();
                let key = chain_name.clone();

                lock.entry(key).or_default().push((n_blocks, lookup_id));
            }
            Trigger::Manual => {}
        }

        // adding it to our lookups is the same, regardless of type
        self.lookup_maps
            .triggers_by_service_workflow
            .write()
            .unwrap()
            .entry(config.service_id.clone())
            .or_default()
            .insert(config.workflow_id.clone(), lookup_id);

        self.lookup_maps
            .trigger_configs
            .write()
            .unwrap()
            .insert(lookup_id, config);
        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_trigger(
        &self,
        service_id: ServiceID,
        workflow_id: WorkflowID,
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
            &mut self.lookup_maps.trigger_configs.write().unwrap(),
            &mut self
                .lookup_maps
                .triggers_by_eth_contract_event
                .write()
                .unwrap(),
            &mut self
                .lookup_maps
                .triggers_by_cosmos_contract_event
                .write()
                .unwrap(),
            &mut self.lookup_maps.triggers_by_block_interval.write().unwrap(),
            lookup_id,
        )?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_service(&self, service_id: wavs_types::ServiceID) -> Result<(), TriggerError> {
        let mut trigger_configs = self.lookup_maps.trigger_configs.write().unwrap();
        let mut triggers_by_eth_contract_event = self
            .lookup_maps
            .triggers_by_eth_contract_event
            .write()
            .unwrap();
        let mut triggers_by_cosmos_contract_event = self
            .lookup_maps
            .triggers_by_cosmos_contract_event
            .write()
            .unwrap();
        let mut triggers_by_block_interval =
            self.lookup_maps.triggers_by_block_interval.write().unwrap();
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
                &mut trigger_configs,
                &mut triggers_by_eth_contract_event,
                &mut triggers_by_cosmos_contract_event,
                &mut triggers_by_block_interval,
                *lookup_id,
            )?;
        }

        // 3. remove from service_workflow_lookup_map
        triggers_by_service_workflow_lock.remove(&service_id);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError> {
        let mut triggers = Vec::new();

        let triggers_by_service_workflow_lock = self
            .lookup_maps
            .triggers_by_service_workflow
            .read()
            .unwrap();
        let trigger_configs = self.lookup_maps.trigger_configs.read().unwrap();

        let workflow_map = triggers_by_service_workflow_lock
            .get(&service_id)
            .ok_or(TriggerError::NoSuchService(service_id))?;

        for lookup_id in workflow_map.values() {
            let trigger_config = trigger_configs
                .get(lookup_id)
                .ok_or(TriggerError::NoSuchTriggerData(*lookup_id))?;
            triggers.push(trigger_config.clone());
        }

        Ok(triggers)
    }
}

fn remove_trigger_data(
    trigger_configs: &mut BTreeMap<usize, TriggerConfig>,
    triggers_by_eth_contract_address: &mut HashMap<
        (ChainName, alloy::primitives::Address, ByteArray<32>),
        HashSet<LookupId>,
    >,
    triggers_by_cosmos_contract_address: &mut HashMap<
        (ChainName, layer_climb::prelude::Address, String),
        HashSet<LookupId>,
    >,
    triggers_by_block_interval: &mut HashMap<ChainName, Vec<(u32, LookupId)>>,
    lookup_id: LookupId,
) -> Result<(), TriggerError> {
    // 1. remove from triggers
    let trigger_data = trigger_configs
        .remove(&lookup_id)
        .ok_or(TriggerError::NoSuchTriggerData(lookup_id))?;

    // 2. remove from contracts
    match trigger_data.trigger {
        Trigger::EthContractEvent {
            address,
            chain_name,
            event_hash,
        } => {
            triggers_by_eth_contract_address
                .remove(&(chain_name.clone(), address, event_hash))
                .ok_or(TriggerError::NoSuchEthContractEvent(
                    chain_name, address, event_hash,
                ))?;
        }
        Trigger::CosmosContractEvent {
            address,
            chain_name,
            event_type,
        } => {
            triggers_by_cosmos_contract_address
                .remove(&(chain_name.clone(), address.clone(), event_type.clone()))
                .ok_or(TriggerError::NoSuchCosmosContractEvent(
                    chain_name, address, event_type,
                ))?;
        }
        Trigger::BlockInterval {
            chain_name,
            n_blocks,
        } => {
            triggers_by_block_interval
                .remove(&chain_name.clone())
                .ok_or(TriggerError::NoSuchBlockIntervalTrigger(
                    chain_name, n_blocks,
                ))?;
        }
        Trigger::Manual => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        apis::trigger::TriggerManager,
        config::Config,
        test_utils::address::{rand_address_eth, rand_event_eth},
    };
    use wavs_types::{ChainName, ServiceID, Trigger, TriggerConfig, WorkflowID};

    use layer_climb::prelude::*;
    use utils::config::{ChainConfigs, CosmosChainConfig, EthereumChainConfig};

    use super::CoreTriggerManager;

    #[test]
    fn core_trigger_lookups() {
        let config = Config {
            active_trigger_chains: vec![ChainName::new("test").unwrap()],
            chains: ChainConfigs {
                eth: [(
                    ChainName::new("test-eth").unwrap(),
                    EthereumChainConfig {
                        chain_id: "eth-local".parse().unwrap(),
                        ws_endpoint: Some("ws://127.0.0.1:26657".to_string()),
                        http_endpoint: Some("http://127.0.0.1:26657".to_string()),
                        aggregator_endpoint: Some("http://127.0.0.1:8001".to_string()),
                        faucet_endpoint: None,
                    },
                )]
                .into_iter()
                .collect(),
                cosmos: [(
                    ChainName::new("test-cosmos").unwrap(),
                    CosmosChainConfig {
                        chain_id: "layer-local".parse().unwrap(),
                        rpc_endpoint: Some("http://127.0.0.1:26657".to_string()),
                        grpc_endpoint: Some("http://127.0.0.1:9090".to_string()),
                        gas_price: 0.025,
                        gas_denom: "uslay".to_string(),
                        bech32_prefix: "layer".to_string(),
                        faucet_endpoint: None,
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

        let trigger_1_1 = TriggerConfig::eth_contract_event(
            &service_id_1,
            &workflow_id_1,
            task_queue_addr_1_1,
            ChainName::new("eth").unwrap(),
            rand_event_eth(),
        )
        .unwrap();
        let trigger_1_2 = TriggerConfig::eth_contract_event(
            &service_id_1,
            &workflow_id_2,
            task_queue_addr_1_2,
            ChainName::new("eth").unwrap(),
            rand_event_eth(),
        )
        .unwrap();
        let trigger_2_1 = TriggerConfig::eth_contract_event(
            &service_id_2,
            &workflow_id_1,
            task_queue_addr_2_1,
            ChainName::new("eth").unwrap(),
            rand_event_eth(),
        )
        .unwrap();
        let trigger_2_2 = TriggerConfig::eth_contract_event(
            &service_id_2,
            &workflow_id_2,
            task_queue_addr_2_2,
            ChainName::new("eth").unwrap(),
            rand_event_eth(),
        )
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
            task_queue_addr_1_1.into()
        );
        assert_eq!(triggers_service_1[1].service_id, service_id_1);
        assert_eq!(triggers_service_1[1].workflow_id, workflow_id_2);
        assert_eq!(
            get_trigger_addr(&triggers_service_1[1].trigger),
            task_queue_addr_1_2.into()
        );

        let triggers_service_2 = manager.list_triggers(service_id_2.clone()).unwrap();

        assert_eq!(triggers_service_2.len(), 2);
        assert_eq!(triggers_service_2[0].service_id, service_id_2);
        assert_eq!(triggers_service_2[0].workflow_id, workflow_id_1);
        assert_eq!(
            get_trigger_addr(&triggers_service_2[0].trigger),
            task_queue_addr_2_1.into()
        );
        assert_eq!(triggers_service_2[1].service_id, service_id_2);
        assert_eq!(triggers_service_2[1].workflow_id, workflow_id_2);
        assert_eq!(
            get_trigger_addr(&triggers_service_2[1].trigger),
            task_queue_addr_2_2.into()
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

        fn get_trigger_addr(trigger: &Trigger) -> Address {
            match trigger {
                Trigger::EthContractEvent { address, .. } => (*address).into(),
                Trigger::CosmosContractEvent { address, .. } => address.clone(),
                _ => panic!("unexpected trigger type"),
            }
        }
    }
}
