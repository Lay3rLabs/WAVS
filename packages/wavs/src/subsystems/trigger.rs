pub mod error;
pub mod lookup;
pub mod schedulers;
pub mod streams;

use crate::{config::Config, dispatcher::TRIGGER_CHANNEL_SIZE, AppContext};
use anyhow::Result;
use error::TriggerError;
use futures::{stream::SelectAll, StreamExt};
use layer_climb::prelude::*;
use lookup::{LookupId, LookupMaps};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::Arc,
};
use streams::{
    cosmos_stream, cron_stream, evm_stream,
    local_command_stream::{self, LocalStreamCommand},
    MultiplexedStream, StreamTriggers,
};
use tokio::sync::mpsc;
use tracing::instrument;
use utils::{
    config::{AnyChainConfig, ChainConfigs},
    evm_client::EvmQueryClient,
    telemetry::TriggerMetrics,
};
use wavs_types::{
    ByteArray, ChainName, ServiceID, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowID,
};

use schedulers::{block_scheduler::BlockIntervalState, cron_scheduler::CronIntervalState};

#[derive(Clone)]
pub struct TriggerManager {
    pub chain_configs: ChainConfigs,
    action_sender: Arc<std::sync::Mutex<Option<mpsc::Sender<TriggerAction>>>>,
    action_receiver: Arc<std::sync::Mutex<Option<mpsc::Receiver<TriggerAction>>>>,
    local_command_sender: Arc<std::sync::Mutex<Option<mpsc::UnboundedSender<LocalStreamCommand>>>>,
    lookup_maps: Arc<LookupMaps>,
    metrics: TriggerMetrics,
    #[cfg(debug_assertions)]
    pub disable_networking: bool,
}

impl TriggerManager {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "TriggerManager"))]
    pub fn new(config: &Config, metrics: TriggerMetrics) -> Result<Self, TriggerError> {
        // TODO - discuss unbounded, crossbeam, etc.
        let (action_sender, action_receiver) = mpsc::channel(TRIGGER_CHANNEL_SIZE);

        Ok(Self {
            chain_configs: config.chains.clone(),
            lookup_maps: Arc::new(LookupMaps::new()),
            action_sender: Arc::new(std::sync::Mutex::new(Some(action_sender))),
            action_receiver: Arc::new(std::sync::Mutex::new(Some(action_receiver))),
            local_command_sender: Arc::new(std::sync::Mutex::new(None)),
            metrics,
            #[cfg(debug_assertions)]
            disable_networking: false,
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn add_trigger(&self, config: TriggerConfig) -> Result<(), TriggerError> {
        // The mechanics of adding a trigger are that we:

        // 1. Setup all the records needed to track the trigger in various "lookup" maps.
        // 2a. If the trigger needs some kind of stream to track it, we need to create that stream.
        // 2b. This happens by way of a "local command" so that everything is handled in `start_watcher` (helps with lifetime issues).
        if let Some(command) = LocalStreamCommand::new(&config) {
            match self.local_command_sender.lock().unwrap().as_ref() {
                Some(sender) => {
                    sender.send(command).unwrap();
                }
                None => {
                    tracing::warn!(
                        "Local command sender not initialized, cannot send command: {:?}",
                        command
                    );
                }
            }
        }

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
                start_block,
                end_block,
            } => {
                self.lookup_maps
                    .block_schedulers
                    .entry(chain_name.clone())
                    .or_default()
                    .add_trigger(BlockIntervalState::new(
                        lookup_id,
                        n_blocks,
                        start_block.map(Into::into),
                        end_block.map(Into::into),
                    ))?;
            }
            Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => {
                // Add directly to the cron scheduler
                self.lookup_maps
                    .cron_scheduler
                    .lock()
                    .unwrap()
                    .add_trigger(CronIntervalState::new(
                        lookup_id, &schedule, start_time, end_time,
                    )?)?;
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

    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "TriggerManager"))]
    pub fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        let action_receiver = self.action_receiver.lock().unwrap().take().unwrap();

        ctx.rt.clone().spawn({
            let _self = self.clone();
            let mut kill_receiver = ctx.get_kill_receiver();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::debug!("Trigger Manager shutting down");
                        // see the note in dispatcher about the channel automatically closing
                        _self.action_sender.lock().unwrap().take();
                    },
                    res = _self.start_watcher() => {
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

    pub async fn send_actions(
        &self,
        trigger_actions: impl IntoIterator<Item = TriggerAction>,
    ) -> Result<(), TriggerError> {
        let action_sender = self.action_sender.lock().unwrap().clone().unwrap();
        for action in trigger_actions {
            action_sender.send(action).await?;
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    async fn start_watcher(&self) -> Result<(), TriggerError> {
        let mut multiplexed_stream: MultiplexedStream = SelectAll::new();

        // Start the local command stream
        let (local_stream_command_sender, local_stream_command_receiver) =
            mpsc::unbounded_channel();
        *self.local_command_sender.lock().unwrap() = Some(local_stream_command_sender);
        let local_command_stream = local_command_stream::start_local_command_stream(
            local_stream_command_receiver,
            self.metrics.clone(),
        )
        .await?;
        multiplexed_stream.push(local_command_stream);

        let mut cosmos_clients = HashMap::new();
        let mut evm_clients = HashMap::new();

        let mut listening_chains = HashSet::new();
        let mut has_started_cron_stream = false;

        // Create a stream for cron triggers that produces a trigger for each due task

        while let Some(res) = multiplexed_stream.next().await {
            let res = match res {
                Err(err) => {
                    tracing::error!("{:?}", err);
                    continue;
                }
                Ok(res) => res,
            };

            tracing::info!("Processing trigger stream event: {:?}", res);
            let mut trigger_actions = Vec::new();

            match res {
                StreamTriggers::LocalCommand(command) => {
                    match command {
                        LocalStreamCommand::StartListeningCron => {
                            #[cfg(debug_assertions)]
                            if self.disable_networking {
                                tracing::warn!(
                                    "Networking is disabled, skipping cron stream start"
                                );
                                continue;
                            }

                            if has_started_cron_stream {
                                tracing::debug!("Cron stream already started, skipping");
                                continue;
                            }

                            has_started_cron_stream = true;

                            let cron_scheduler = self.lookup_maps.cron_scheduler.clone();
                            match cron_stream::start_cron_stream(
                                cron_scheduler,
                                self.metrics.clone(),
                            )
                            .await
                            {
                                Ok(cron_stream) => {
                                    multiplexed_stream.push(cron_stream);
                                }
                                Err(err) => {
                                    tracing::error!("Failed to start cron stream: {:?}", err);
                                    continue;
                                }
                            }
                        }
                        LocalStreamCommand::StartListeningChain { chain_name } => {
                            #[cfg(debug_assertions)]
                            if self.disable_networking {
                                tracing::warn!(
                                    "Networking is disabled, skipping chain stream start"
                                );
                                continue;
                            }
                            if listening_chains.contains(&chain_name) {
                                tracing::debug!("Already listening to chain {}", chain_name);
                                continue;
                            }

                            // insert right away, before we get to an await point
                            listening_chains.insert(chain_name.clone());

                            let chain_config = match self.chain_configs.get_chain(&chain_name) {
                                Ok(config) => match config {
                                    Some(config) => config,
                                    None => {
                                        tracing::error!("No chain config found for {}", chain_name);
                                        continue;
                                    }
                                },
                                Err(e) => {
                                    tracing::error!("{:?}", e);
                                    continue;
                                }
                            };

                            match chain_config {
                                AnyChainConfig::Cosmos(chain_config) => {
                                    let cosmos_client =
                                        QueryClient::new(chain_config.clone().into(), None)
                                            .await
                                            .map_err(TriggerError::Climb)?;

                                    cosmos_clients
                                        .insert(chain_name.clone(), cosmos_client.clone());

                                    // Start the Cosmos event stream
                                    match cosmos_stream::start_cosmos_stream(
                                        cosmos_client.clone(),
                                        chain_name.clone(),
                                        self.metrics.clone(),
                                    )
                                    .await
                                    {
                                        Ok(cosmos_event_stream) => {
                                            multiplexed_stream.push(cosmos_event_stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to start Cosmos event stream: {:?}",
                                                err
                                            );
                                            continue;
                                        }
                                    }
                                }
                                AnyChainConfig::Evm(chain_config) => {
                                    let endpoint =
                                        chain_config.query_client_endpoint().map_err(|e| {
                                            TriggerError::EvmClient(chain_name.clone(), e)
                                        })?;
                                    let evm_client =
                                        EvmQueryClient::new(endpoint).await.map_err(|e| {
                                            TriggerError::EvmClient(chain_name.clone(), e)
                                        })?;

                                    evm_clients.insert(chain_name.clone(), evm_client.clone());

                                    // Start the EVM event stream
                                    match evm_stream::start_evm_event_stream(
                                        evm_client.clone(),
                                        chain_name.clone(),
                                        self.metrics.clone(),
                                    )
                                    .await
                                    {
                                        Ok(evm_event_stream) => {
                                            multiplexed_stream.push(evm_event_stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to start EVM event stream: {:?}",
                                                err
                                            );
                                            continue;
                                        }
                                    }

                                    // Start the EVM block stream
                                    match evm_stream::start_evm_block_stream(
                                        evm_client.clone(),
                                        chain_name.clone(),
                                        self.metrics.clone(),
                                    )
                                    .await
                                    {
                                        Ok(evm_block_stream) => {
                                            multiplexed_stream.push(evm_block_stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to start EVM block stream: {:?}",
                                                err
                                            );
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
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

            if !trigger_actions.is_empty() {
                tracing::info!(
                    "Sending {} trigger actions to dispatcher",
                    trigger_actions.len()
                );
                for (idx, action) in trigger_actions.iter().enumerate() {
                    tracing::debug!(
                        "Trigger action (in this batch) {}: service_id={}, workflow_id={}, trigger_data={:?}",
                        idx + 1,
                        action.config.service_id,
                        action.config.workflow_id,
                        action.data
                    );
                }

                self.send_actions(trigger_actions).await?;
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
    }

    /// Process blocks and return trigger actions for any triggers that should fire
    pub fn process_blocks(&self, chain_name: ChainName, block_height: u64) -> Vec<TriggerAction> {
        let block_height = match NonZeroU64::new(block_height) {
            Some(height) => height,
            None => {
                self.metrics.increment_total_errors("block height is zero");
                return Vec::new();
            }
        };
        // Get the triggers that should fire at this block height
        let firing_lookup_ids = match self.lookup_maps.block_schedulers.get_mut(&chain_name) {
            Some(mut scheduler) => scheduler.tick(block_height.into()),
            None => Vec::new(),
        };

        // Convert lookup_ids to TriggerActions
        if !firing_lookup_ids.is_empty() {
            let trigger_configs_lock = self.lookup_maps.trigger_configs.read().unwrap();

            let mut trigger_actions = Vec::with_capacity(firing_lookup_ids.len());

            for lookup_id in firing_lookup_ids {
                if let Some(trigger_config) = trigger_configs_lock.get(&lookup_id) {
                    trigger_actions.push(TriggerAction {
                        data: TriggerData::BlockInterval {
                            chain_name: chain_name.clone(),
                            block_height: block_height.get(),
                        },
                        config: trigger_config.clone(),
                    });
                } else {
                    self.metrics
                        .increment_total_errors("block interval trigger config not found");
                    tracing::warn!("Missing trigger config for block interval id {}", lookup_id);
                }
            }

            trigger_actions
        } else {
            Vec::new()
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn remove_trigger(
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

        // Get the trigger type to know which scheduler to remove from
        let trigger_type = {
            let trigger_configs = self.lookup_maps.trigger_configs.read().unwrap();
            trigger_configs
                .get(&lookup_id)
                .map(|config| config.trigger.clone())
        };

        // Remove from the appropriate collection based on trigger type
        if let Some(trigger) = trigger_type {
            match trigger {
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
                    if let Some(set) = lock.get_mut(&(chain_name.clone(), address, event_hash)) {
                        set.remove(&lookup_id);
                        if set.is_empty() {
                            lock.remove(&(chain_name, address, event_hash));
                        }
                    }
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
                    if let Some(set) =
                        lock.get_mut(&(chain_name.clone(), address.clone(), event_type.clone()))
                    {
                        set.remove(&lookup_id);
                        if set.is_empty() {
                            lock.remove(&(chain_name, address, event_type));
                        }
                    }
                }
                Trigger::BlockInterval { chain_name, .. } => {
                    // Remove from block scheduler
                    if let Some(mut scheduler) =
                        self.lookup_maps.block_schedulers.get_mut(&chain_name)
                    {
                        scheduler.remove_trigger(lookup_id);
                    }
                }
                Trigger::Cron { .. } => {
                    // Remove from cron scheduler
                    self.lookup_maps
                        .cron_scheduler
                        .lock()
                        .unwrap()
                        .remove_trigger(lookup_id);
                }
                Trigger::Manual => {}
            }
        }

        // Remove from trigger_configs
        self.lookup_maps
            .trigger_configs
            .write()
            .unwrap()
            .remove(&lookup_id);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn remove_service(&self, service_id: wavs_types::ServiceID) -> Result<(), TriggerError> {
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

        // Collect all lookup IDs to be removed
        let lookup_ids: Vec<LookupId> = workflow_map.values().copied().collect();

        // Remove triggers from all collections
        for lookup_id in &lookup_ids {
            if let Some(config) = trigger_configs.get(lookup_id) {
                match &config.trigger {
                    Trigger::EvmContractEvent {
                        address,
                        chain_name,
                        event_hash,
                    } => {
                        if let Some(set) = triggers_by_evm_contract_event.get_mut(&(
                            chain_name.clone(),
                            *address,
                            *event_hash,
                        )) {
                            set.remove(lookup_id);
                            if set.is_empty() {
                                triggers_by_evm_contract_event.remove(&(
                                    chain_name.clone(),
                                    *address,
                                    *event_hash,
                                ));
                            }
                        }
                    }
                    Trigger::CosmosContractEvent {
                        address,
                        chain_name,
                        event_type,
                    } => {
                        if let Some(set) = triggers_by_cosmos_contract_event.get_mut(&(
                            chain_name.clone(),
                            address.clone(),
                            event_type.clone(),
                        )) {
                            set.remove(lookup_id);
                            if set.is_empty() {
                                triggers_by_cosmos_contract_event.remove(&(
                                    chain_name.clone(),
                                    address.clone(),
                                    event_type.clone(),
                                ));
                            }
                        }
                    }
                    Trigger::BlockInterval { chain_name, .. } => {
                        // Remove from block scheduler
                        if let Some(mut scheduler) =
                            self.lookup_maps.block_schedulers.get_mut(chain_name)
                        {
                            scheduler.remove_trigger(*lookup_id);
                        }
                    }
                    Trigger::Cron { .. } => {
                        self.lookup_maps
                            .cron_scheduler
                            .lock()
                            .unwrap()
                            .remove_trigger(*lookup_id);
                    }
                    Trigger::Manual => {}
                }
            }
        }

        // Remove all trigger configs
        for lookup_id in &lookup_ids {
            trigger_configs.remove(lookup_id);
        }

        // Remove from service_workflow_lookup_map
        triggers_by_service_workflow_lock.remove(&service_id);

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError> {
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

    #[cfg(debug_assertions)]
    pub fn get_lookup_maps(&self) -> &Arc<LookupMaps> {
        &self.lookup_maps
    }
}

#[cfg(test)]
mod tests {}
