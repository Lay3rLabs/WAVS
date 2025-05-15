use crate::{
    apis::trigger::{TriggerError, TriggerManager},
    config::Config,
    AppContext,
};
use alloy_provider::Provider;
use alloy_rpc_types_eth::{Filter, Log};
use anyhow::{Context, Result};
use futures::{Stream, StreamExt};
use layer_climb::prelude::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::IntervalStream;
use tracing::instrument;
use utils::{config::AnyChainConfig, evm_client::EvmQueryClient, telemetry::TriggerMetrics};
use wavs_types::{
    ByteArray, ChainName, ServiceID, Timestamp, Trigger, TriggerAction, TriggerConfig, TriggerData,
    WorkflowID,
};

use super::cron_scheduler::CronScheduler;

#[derive(Clone)]
pub struct CoreTriggerManager {
    pub chain_configs: HashMap<ChainName, AnyChainConfig>,
    pub channel_bound: usize,
    lookup_maps: Arc<LookupMaps>,
    metrics: TriggerMetrics,
}

#[allow(clippy::type_complexity)]
struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    pub trigger_configs: Arc<RwLock<BTreeMap<LookupId, TriggerConfig>>>,
    /// lookup id by (chain name, contract event address, event type)
    pub triggers_by_cosmos_contract_event:
        Arc<RwLock<HashMap<(ChainName, layer_climb::prelude::Address, String), HashSet<LookupId>>>>,
    /// lookup id by (chain id, contract event address, event hash)
    pub triggers_by_evm_contract_event: Arc<
        RwLock<HashMap<(ChainName, alloy_primitives::Address, ByteArray<32>), HashSet<LookupId>>>,
    >,
    /// lookup by chain_name -> n_blocks
    pub triggers_by_block_interval: Arc<RwLock<HashMap<ChainName, Vec<BlockIntervalSchedule>>>>,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow:
        Arc<RwLock<BTreeMap<ServiceID, BTreeMap<WorkflowID, LookupId>>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
    /// cron scheduler
    pub cron_scheduler: CronScheduler,
}

struct BlockIntervalSchedule {
    /// countdown to the next trigger
    countdown: u32,
    /// lookup id
    lookup_id: LookupId,
}
impl BlockIntervalSchedule {
    fn new(lookup_id: LookupId, countdown: u32) -> Self {
        Self {
            countdown,
            lookup_id,
        }
    }
}

impl LookupMaps {
    pub fn new() -> Self {
        Self {
            trigger_configs: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            triggers_by_cosmos_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_evm_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_block_interval: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_service_workflow: Arc::new(RwLock::new(BTreeMap::new())),
            cron_scheduler: CronScheduler::default(),
        }
    }
}

pub(crate) type LookupId = usize;

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
    Evm {
        chain_name: ChainName,
        log: Log,
        block_height: u64,
    },
    // We need a separate stream for EVM block interval triggers
    EvmBlock {
        chain_name: ChainName,
        block_height: u64,
    },
    Cron {
        /// Unix timestamp (in nanos) when these triggers were processed
        trigger_time: Timestamp,
        /// Vector of lookup IDs for all triggers that are due at this time.
        /// Multiple triggers can fire simultaneously in a single tick.
        lookup_ids: Vec<LookupId>,
    },
}

impl CoreTriggerManager {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "TriggerManager"))]
    pub fn new(config: &Config, metrics: TriggerMetrics) -> Result<Self, TriggerError> {
        Ok(Self {
            chain_configs: config.active_trigger_chain_configs(),
            channel_bound: 100, // TODO: get from config
            lookup_maps: Arc::new(LookupMaps::new()),
            metrics,
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

        let mut evm_clients = HashMap::new();
        for (chain_name, chain_config) in self.chain_configs.clone() {
            if let AnyChainConfig::Evm(chain_config) = chain_config {
                let endpoint = chain_config
                    .query_client_endpoint()
                    .map_err(|e| TriggerError::EvmClient(chain_name.clone(), e))?;
                let client = EvmQueryClient::new(endpoint)
                    .await
                    .map_err(|e| TriggerError::EvmClient(chain_name.clone(), e))?;

                evm_clients.insert(chain_name, client);
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
                        Err(err) => {
                            self.metrics.increment_total_errors("block_events");
                            Err(err)
                        }
                    }),
            );

            streams.push(event_stream);
        }

        for (chain_name, query_client) in evm_clients.iter() {
            tracing::debug!("Trigger Manager for EVM chain {} started", chain_name);

            // Start the event stream
            let filter = Filter::new();

            let stream = query_client
                .provider
                .subscribe_logs(&filter)
                .await
                .map_err(|e| TriggerError::EvmSubscription(e.into()))?
                .into_stream();

            let chain_name = chain_name.clone();

            let event_stream = Box::pin(stream.map(move |log| {
                Ok(StreamTriggers::Evm {
                    chain_name: chain_name.clone(),
                    block_height: log.block_number.context("couldn't get EVM block height")?,
                    log,
                })
            }));

            streams.push(event_stream);
        }

        for (chain_name, query_client) in evm_clients.iter() {
            let chain_name = chain_name.clone();

            // Start the block stream (for block-based triggers)
            let stream = query_client
                .provider
                .subscribe_blocks()
                .await
                .map_err(|e| TriggerError::EvmSubscription(e.into()))?
                .into_stream();

            let block_stream = Box::pin(stream.map(move |block| {
                Ok(StreamTriggers::EvmBlock {
                    chain_name: chain_name.clone(),
                    block_height: block.number,
                })
            }));
            streams.push(block_stream);
        }

        // Create a stream for cron triggers that produces a trigger for each due task
        let cron_scheduler = self.lookup_maps.cron_scheduler.clone();
        let interval_stream =
            IntervalStream::new(tokio::time::interval(std::time::Duration::from_secs(1)));

        // Process cron triggers on each interval tick
        let cron_stream = Box::pin(interval_stream.map(move |_| {
            let trigger_time = Timestamp::now();
            let lookup_ids = cron_scheduler.process_due_triggers(trigger_time);

            Ok(StreamTriggers::Cron {
                lookup_ids,
                trigger_time,
            })
        }));

        streams.push(cron_stream);

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
                StreamTriggers::Evm {
                    log,
                    chain_name,
                    block_height,
                } => {
                    if let Some(event_hash) = log.topic0() {
                        let contract_address = log.address();

                        let triggers_by_contract_event_lock = self
                            .lookup_maps
                            .triggers_by_evm_contract_event
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
                                            data: TriggerData::EvmContractEvent {
                                                contract_address,
                                                chain_name: chain_name.clone(),
                                                log: log.inner.data.clone(),
                                                block_height,
                                            },
                                            config: trigger_config.clone(),
                                        });
                                    }
                                    None => {
                                        self.metrics.increment_total_errors(
                                            "evm event trigger config not found",
                                        );
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
                    // extra scope in order to properly drop the locks
                    {
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
                                            self.metrics.increment_total_errors(
                                                "cosmos event trigger config not found",
                                            );
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

                    // process block-based triggers
                    trigger_actions.extend(self.process_blocks(chain_name, block_height));
                }
                StreamTriggers::EvmBlock {
                    chain_name,
                    block_height,
                } => {
                    trigger_actions.extend(self.process_blocks(chain_name, block_height));
                }
                StreamTriggers::Cron {
                    trigger_time,
                    lookup_ids,
                } => {
                    let trigger_configs_lock = self.lookup_maps.trigger_configs.read().unwrap();

                    for lookup_id in lookup_ids {
                        match trigger_configs_lock.get(&lookup_id) {
                            Some(trigger_config) => {
                                trigger_actions.push(TriggerAction {
                                    data: TriggerData::Cron { trigger_time },
                                    config: trigger_config.clone(),
                                });
                            }
                            None => {
                                self.metrics
                                    .increment_total_errors("cron trigger config not found");
                                tracing::warn!(
                                    "Trigger config not found for cron lookup_id {}",
                                    lookup_id
                                );
                            }
                        }
                    }
                }
            }

            for action in trigger_actions {
                action_sender.send(action).await.unwrap();
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
    }

    fn process_blocks(&self, chain_name: ChainName, block_height: u64) -> Vec<TriggerAction> {
        let mut finished = Vec::new();
        let mut explicit_started = Vec::new();
        let mut trigger_actions = vec![];
        {
            let mut triggers_by_block_interval_lock =
                self.lookup_maps.triggers_by_block_interval.write().unwrap();
            let trigger_configs_lock = self.lookup_maps.trigger_configs.read().unwrap();

            if let Some(triggers) = triggers_by_block_interval_lock.get_mut(&chain_name) {
                // Since we don't remove the trigger data when the trigger config is removed,
                // for efficiency we want to do it here.

                triggers.retain(|interval| trigger_configs_lock.contains_key(&interval.lookup_id));

                // now we can iterate again on the active triggers
                for interval in triggers.iter_mut() {
                    // safe - we just ensured that the trigger config exists
                    let trigger_config = trigger_configs_lock.get(&interval.lookup_id).unwrap();

                    if let Trigger::BlockInterval {
                        n_blocks,
                        start_block,
                        end_block,
                        ..
                    } = trigger_config.trigger
                    {
                        if let Some(start_block) = start_block {
                            let start_block: u64 = start_block.into();
                            match block_height.cmp(&start_block) {
                                std::cmp::Ordering::Less => {
                                    // we haven't started yet
                                    continue;
                                }
                                std::cmp::Ordering::Greater => {
                                    // missed the window somehow :(
                                    tracing::error!("Block interval trigger {} missed the window! start block is {} but current block is {}",
                                        interval.lookup_id, start_block, block_height);
                                    finished.push(interval.lookup_id);
                                    continue;
                                }
                                std::cmp::Ordering::Equal => {
                                    // we've hit the target block
                                    // go ahead and start it, and we'll set the start block to None
                                    // so that we can catch errors for missed windows
                                    explicit_started.push(interval.lookup_id);
                                }
                            }
                        }

                        if let Some(end_block) = end_block {
                            if block_height >= end_block.into() {
                                finished.push(interval.lookup_id);
                                continue;
                            }
                        }

                        // decrement the countdown
                        interval.countdown -= 1;

                        if interval.countdown == 0 {
                            // reset the countdown to n_blocks
                            interval.countdown = n_blocks.into();
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
            }
        }

        // we've dropped the read lock on trigger configs by here, so we can manipulate the table
        // no need to proactively remove them from the interval table, however, that'll happen naturally like explicit remove
        if !finished.is_empty() || !explicit_started.is_empty() {
            let mut lock = self.lookup_maps.trigger_configs.write().unwrap();
            // these have now started, so we can remove the start block
            for id in explicit_started.drain(..) {
                if let Trigger::BlockInterval { start_block, .. } =
                    &mut lock.get_mut(&id).unwrap().trigger
                {
                    *start_block = None;
                }
            }
            // these are done, so we can remove them
            for id in finished.drain(..) {
                lock.remove(&id);
            }
        }

        trigger_actions
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
                    res = _self.start_watcher(action_sender) => {
                        if let Err(err) = res {
                            tracing::error!("Trigger Manager watcher error: {:?}", err);
                            ctx.kill();
                        }
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
            Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } => {
                let mut lock = self
                    .lookup_maps
                    .triggers_by_evm_contract_event
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
                start_block: _,
                end_block: _,
            } => {
                self.lookup_maps
                    .triggers_by_block_interval
                    .write()
                    .unwrap()
                    .entry(chain_name.clone())
                    .or_default()
                    .push(BlockIntervalSchedule::new(lookup_id, n_blocks.into()));
            }
            Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => {
                // Add directly to the cron scheduler
                self.lookup_maps
                    .cron_scheduler
                    .add_trigger(lookup_id, schedule, start_time, end_time)?;
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
                .triggers_by_evm_contract_event
                .write()
                .unwrap(),
            &mut self
                .lookup_maps
                .triggers_by_cosmos_contract_event
                .write()
                .unwrap(),
            &self.lookup_maps.cron_scheduler,
            lookup_id,
        )?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_service(&self, service_id: wavs_types::ServiceID) -> Result<(), TriggerError> {
        let mut trigger_configs = self.lookup_maps.trigger_configs.write().unwrap();
        let mut triggers_by_evm_contract_event = self
            .lookup_maps
            .triggers_by_evm_contract_event
            .write()
            .unwrap();
        let mut triggers_by_cosmos_contract_event = self
            .lookup_maps
            .triggers_by_cosmos_contract_event
            .write()
            .unwrap();
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
                &mut triggers_by_evm_contract_event,
                &mut triggers_by_cosmos_contract_event,
                &self.lookup_maps.cron_scheduler,
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
    triggers_by_evm_contract_address: &mut HashMap<
        (ChainName, alloy_primitives::Address, ByteArray<32>),
        HashSet<LookupId>,
    >,
    triggers_by_cosmos_contract_address: &mut HashMap<
        (ChainName, layer_climb::prelude::Address, String),
        HashSet<LookupId>,
    >,
    cron_scheduler: &CronScheduler,
    lookup_id: LookupId,
) -> Result<(), TriggerError> {
    // 1. remove from triggers
    let trigger_data = trigger_configs
        .remove(&lookup_id)
        .ok_or(TriggerError::NoSuchTriggerData(lookup_id))?;

    // 2. remove from contracts
    match trigger_data.trigger {
        Trigger::EvmContractEvent {
            address,
            chain_name,
            event_hash,
        } => {
            triggers_by_evm_contract_address
                .remove(&(chain_name.clone(), address, event_hash))
                .ok_or(TriggerError::NoSuchEvmContractEvent(
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
        Trigger::BlockInterval { .. } => {
            // after being removed from the trigger config, actual trigger is deleted
            // during the block processing
        }
        Trigger::Cron { .. } => {
            // Remove from cron scheduler - errors are already handled inside
            cron_scheduler.remove_trigger(lookup_id)?;
        }
        Trigger::Manual => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use crate::{
        apis::trigger::TriggerManager,
        config::Config,
        test_utils::address::{rand_address_evm, rand_event_evm},
    };
    use wavs_types::{ChainName, ServiceID, Timestamp, Trigger, TriggerConfig, WorkflowID};

    use layer_climb::prelude::*;
    use utils::{
        config::{ChainConfigs, CosmosChainConfig, EvmChainConfig},
        telemetry::TriggerMetrics,
    };

    use super::CoreTriggerManager;

    #[test]
    fn core_trigger_lookups() {
        let config = Config {
            active_trigger_chains: vec![ChainName::new("test").unwrap()],
            chains: ChainConfigs {
                evm: [(
                    ChainName::new("test-evm").unwrap(),
                    EvmChainConfig {
                        chain_id: "evm-local".parse().unwrap(),
                        ws_endpoint: Some("ws://127.0.0.1:26657".to_string()),
                        http_endpoint: Some("http://127.0.0.1:26657".to_string()),
                        aggregator_endpoint: Some("http://127.0.0.1:8001".to_string()),
                        faucet_endpoint: None,
                        poll_interval_ms: None,
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

        let manager = CoreTriggerManager::new(
            &config,
            TriggerMetrics::new(&opentelemetry::global::meter("trigger-test-metrics")),
        )
        .unwrap();

        let service_id_1 = ServiceID::new("service-1").unwrap();
        let workflow_id_1 = WorkflowID::new("workflow-1").unwrap();

        let service_id_2 = ServiceID::new("service-2").unwrap();
        let workflow_id_2 = WorkflowID::new("workflow-2").unwrap();

        let task_queue_addr_1_1 = rand_address_evm();
        let task_queue_addr_1_2 = rand_address_evm();
        let task_queue_addr_2_1 = rand_address_evm();
        let task_queue_addr_2_2 = rand_address_evm();

        let trigger_1_1 = TriggerConfig::evm_contract_event(
            &service_id_1,
            &workflow_id_1,
            task_queue_addr_1_1,
            ChainName::new("evm").unwrap(),
            rand_event_evm(),
        )
        .unwrap();
        let trigger_1_2 = TriggerConfig::evm_contract_event(
            &service_id_1,
            &workflow_id_2,
            task_queue_addr_1_2,
            ChainName::new("evm").unwrap(),
            rand_event_evm(),
        )
        .unwrap();
        let trigger_2_1 = TriggerConfig::evm_contract_event(
            &service_id_2,
            &workflow_id_1,
            task_queue_addr_2_1,
            ChainName::new("evm").unwrap(),
            rand_event_evm(),
        )
        .unwrap();
        let trigger_2_2 = TriggerConfig::evm_contract_event(
            &service_id_2,
            &workflow_id_2,
            task_queue_addr_2_2,
            ChainName::new("evm").unwrap(),
            rand_event_evm(),
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
                Trigger::EvmContractEvent { address, .. } => (*address).into(),
                Trigger::CosmosContractEvent { address, .. } => address.clone(),
                _ => panic!("unexpected trigger type"),
            }
        }
    }

    #[tokio::test]
    async fn block_interval_trigger_is_removed_when_config_is_gone() {
        let config = Config {
            active_trigger_chains: vec![ChainName::new("test").unwrap()],
            chains: ChainConfigs {
                evm: [(
                    ChainName::new("test-evm").unwrap(),
                    EvmChainConfig {
                        chain_id: "evm-local".parse().unwrap(),
                        ws_endpoint: Some("ws://127.0.0.1:26657".to_string()),
                        http_endpoint: Some("http://127.0.0.1:26657".to_string()),
                        aggregator_endpoint: Some("http://127.0.0.1:8001".to_string()),
                        faucet_endpoint: None,
                        poll_interval_ms: None,
                    },
                )]
                .into_iter()
                .collect(),
                cosmos: Default::default(),
            },
            ..Default::default()
        };

        let manager = CoreTriggerManager::new(
            &config,
            TriggerMetrics::new(&opentelemetry::global::meter("trigger-test-metrics")),
        )
        .unwrap();

        let service_id = ServiceID::new("service-1").unwrap();
        let workflow_id = WorkflowID::new("workflow-1").unwrap();
        let chain_name = ChainName::new("evm").unwrap();

        // set number of blocks to 1 to fire the trigger immediately
        let n_blocks = NonZero::new(1).unwrap();
        let trigger = TriggerConfig::block_interval_event(
            &service_id,
            &workflow_id,
            chain_name.clone(),
            n_blocks,
        )
        .unwrap();
        manager.add_trigger(trigger.clone()).unwrap();

        let service_id2 = ServiceID::new("service-2").unwrap();
        let trigger = TriggerConfig::block_interval_event(
            &service_id2,
            &workflow_id,
            chain_name.clone(),
            n_blocks,
        )
        .unwrap();
        manager.add_trigger(trigger.clone()).unwrap();

        // verify that triggers exist in the lookup maps
        {
            let triggers_by_block_interval_lock = manager
                .lookup_maps
                .triggers_by_block_interval
                .read()
                .unwrap();
            let countdowns = triggers_by_block_interval_lock.get(&chain_name).unwrap();
            assert_eq!(countdowns.len(), 2);
        }

        manager
            .remove_trigger(service_id.clone(), workflow_id.clone())
            .unwrap();

        let trigger_actions = manager.process_blocks(chain_name.clone(), 10);

        // verify only one trigger action is generated
        assert_eq!(trigger_actions.len(), 1);

        // verify the trigger data is removed from the lookup maps
        {
            let triggers_by_block_interval_lock = manager
                .lookup_maps
                .triggers_by_block_interval
                .read()
                .unwrap();
            let countdowns = triggers_by_block_interval_lock.get(&chain_name).unwrap();
            assert_eq!(countdowns.len(), 1);
        }

        // remove the last trigger config
        manager
            .remove_trigger(service_id2.clone(), workflow_id.clone())
            .unwrap();

        let trigger_actions = manager.process_blocks(chain_name.clone(), 10);

        // verify no trigger action is generated this time
        assert!(trigger_actions.is_empty());

        // verify the trigger data is now empty
        {
            let triggers_by_block_interval_lock = manager
                .lookup_maps
                .triggers_by_block_interval
                .read()
                .unwrap();
            let countdowns = triggers_by_block_interval_lock.get(&chain_name).unwrap();
            assert!(countdowns.is_empty());
        }
    }

    #[tokio::test]
    async fn cron_trigger_is_removed_when_config_is_gone() {
        // Setup configuration and manager
        let config = Config {
            active_trigger_chains: vec![ChainName::new("test").unwrap()],
            ..Default::default()
        };

        let manager = CoreTriggerManager::new(
            &config,
            TriggerMetrics::new(&opentelemetry::global::meter("trigger-test-metrics")),
        )
        .unwrap();

        // Create service and workflow IDs
        let service_id = ServiceID::new("service-1").unwrap();
        let workflow_id = WorkflowID::new("workflow-1").unwrap();

        // Set up the first trigger
        let trigger1 = TriggerConfig {
            service_id: service_id.clone(),
            workflow_id: workflow_id.clone(),
            trigger: Trigger::Cron {
                schedule: "* * * * * *".to_owned(),
                start_time: None,
                end_time: None,
            },
        };
        manager.add_trigger(trigger1).unwrap();

        // Set up the second trigger
        let service_id2 = ServiceID::new("service-2").unwrap();
        let trigger2 = TriggerConfig {
            service_id: service_id2.clone(),
            workflow_id: workflow_id.clone(),
            trigger: Trigger::Cron {
                schedule: "* * * * * *".to_owned(),
                start_time: None,
                end_time: None,
            },
        };
        manager.add_trigger(trigger2).unwrap();

        // Use a future time to process triggers
        let future_time =
            Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
        let lookup_ids = manager
            .lookup_maps
            .cron_scheduler
            .process_due_triggers(future_time);

        // Verify both triggers fire
        assert_eq!(lookup_ids.len(), 2, "Expected 2 triggers to fire");

        // Remove the first trigger
        manager
            .remove_trigger(service_id.clone(), workflow_id.clone())
            .unwrap();

        // Process triggers again
        let future_time =
            Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
        let lookup_ids = manager
            .lookup_maps
            .cron_scheduler
            .process_due_triggers(future_time);

        // Verify only one trigger fires now
        assert_eq!(
            lookup_ids.len(),
            1,
            "Expected 1 trigger to fire after removing one"
        );

        // Remove the second trigger
        manager
            .remove_trigger(service_id2.clone(), workflow_id.clone())
            .unwrap();

        // Process triggers one more time
        let future_time =
            Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10)).unwrap();
        let lookup_ids = manager
            .lookup_maps
            .cron_scheduler
            .process_due_triggers(future_time);

        // Verify no triggers fire
        assert!(
            lookup_ids.is_empty(),
            "Expected no triggers to fire after removing all"
        );
    }
}
