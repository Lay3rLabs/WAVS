pub mod error;
pub mod lookup;
pub mod schedulers;
pub mod streams;

use crate::{
    config::Config,
    dispatcher::{DispatcherCommand, TRIGGER_CHANNEL_SIZE},
    services::Services,
    AppContext,
};
use alloy_sol_types::SolEvent;
use anyhow::Result;
use error::TriggerError;
use futures::{stream::SelectAll, StreamExt};
use layer_climb::prelude::*;
use lookup::LookupMaps;
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
    config::{AnyChainConfig, ChainConfigs, EvmChainConfigExt},
    evm_client::EvmQueryClient,
    telemetry::TriggerMetrics,
};
use wavs_types::{
    ByteArray, ChainName, IWavsServiceManager, ServiceID, TriggerAction, TriggerConfig, TriggerData,
};

#[derive(Clone)]
pub struct TriggerManager {
    pub chain_configs: Arc<std::sync::RwLock<ChainConfigs>>,
    dispatcher_command_sender: Arc<std::sync::Mutex<Option<mpsc::Sender<DispatcherCommand>>>>,
    dispatcher_command_receiver: Arc<std::sync::Mutex<Option<mpsc::Receiver<DispatcherCommand>>>>,
    local_command_sender: Arc<std::sync::Mutex<Option<mpsc::UnboundedSender<LocalStreamCommand>>>>,
    lookup_maps: Arc<LookupMaps>,
    metrics: TriggerMetrics,
    #[cfg(debug_assertions)]
    pub disable_networking: bool,
    pub services: Services,
}

impl TriggerManager {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", skip(services), fields(subsys = "TriggerManager"))]
    pub fn new(
        config: &Config,
        metrics: TriggerMetrics,
        services: Services,
    ) -> Result<Self, TriggerError> {
        // TODO - discuss unbounded, crossbeam, etc.
        let (dispatcher_command_sender, dispatcher_command_receiver) =
            mpsc::channel(TRIGGER_CHANNEL_SIZE);

        Ok(Self {
            chain_configs: Arc::new(std::sync::RwLock::new(config.chains.clone())),
            lookup_maps: Arc::new(LookupMaps::new(services.clone(), metrics.clone())),
            dispatcher_command_sender: Arc::new(std::sync::Mutex::new(Some(
                dispatcher_command_sender,
            ))),
            dispatcher_command_receiver: Arc::new(std::sync::Mutex::new(Some(
                dispatcher_command_receiver,
            ))),
            local_command_sender: Arc::new(std::sync::Mutex::new(None)),
            metrics,
            #[cfg(debug_assertions)]
            disable_networking: false,
            services,
        })
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn add_service(&self, service: &wavs_types::Service) -> Result<(), TriggerError> {
        // The mechanics of adding a trigger are that we:

        // 1. Setup all the records needed to track the trigger in various "lookup" maps.
        // 2a. If the trigger needs some kind of stream to kick it off, we need to create that stream.
        // 2b. Actual stream-creation happens by way of a "local command" so that everything is handled in `start_watcher` (helps with lifetime issues).
        //
        // It doesn't really matter what order the multiplexed streams are polled in, a trigger simply
        // will not be fired until the stream that kicks it off is polled (i.e. this definitively happens _after_ the stream is created).

        self.lookup_maps.add_service(service)?;

        match self.local_command_sender.lock().unwrap().as_ref() {
            Some(sender) => {
                // Ensure the service manager's chain is being listened to for service change events
                // This is needed even if the service has no workflows, so service URI changes can be detected
                sender.send(LocalStreamCommand::StartListeningChain {
                    chain_name: service.manager.chain_name().clone(),
                })?;
            }
            None => {
                tracing::warn!(
                    "Local command sender not initialized, cannot send command for service manager chain: {:?}",
                    service.manager.chain_name()
                );
            }
        }

        for (id, workflow) in &service.workflows {
            let config = TriggerConfig {
                service_id: service.id(),
                workflow_id: id.clone(),
                trigger: workflow.trigger.clone(),
            };

            if let Some(command) = LocalStreamCommand::new(&config) {
                match self.local_command_sender.lock().unwrap().as_ref() {
                    Some(sender) => {
                        sender.send(command)?;
                    }
                    None => {
                        tracing::warn!(
                            "Local command sender not initialized, cannot send command: {:?}",
                            command
                        );
                    }
                }
            }
        }

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub fn remove_service(&self, service_id: ServiceID) -> Result<(), TriggerError> {
        self.lookup_maps.remove_service(service_id.clone())
    }

    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "TriggerManager"))]
    pub fn start(
        &self,
        ctx: AppContext,
    ) -> Result<mpsc::Receiver<DispatcherCommand>, TriggerError> {
        let dispatcher_command_receiver = self
            .dispatcher_command_receiver
            .lock()
            .unwrap()
            .take()
            .unwrap();

        ctx.rt.clone().spawn({
            let _self = self.clone();
            let mut kill_receiver = ctx.get_kill_receiver();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::debug!("Trigger Manager shutting down");
                        // see the note in dispatcher about the channel automatically closing
                        _self.dispatcher_command_sender.lock().unwrap().take();
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

        Ok(dispatcher_command_receiver)
    }

    pub async fn send_dispatcher_commands(
        &self,
        commands: impl IntoIterator<Item = DispatcherCommand>,
    ) -> Result<(), TriggerError> {
        let dispatcher_command_sender = self
            .dispatcher_command_sender
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        for command in commands {
            dispatcher_command_sender.send(command).await?;
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
            let mut dispatcher_commands = Vec::new();

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

                            let chain_config =
                                match self.chain_configs.read().unwrap().get_chain(&chain_name) {
                                    Ok(config) => match config {
                                        Some(config) => config,
                                        None => {
                                            tracing::error!(
                                                "No chain config found for {}",
                                                chain_name
                                            );
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
                                    let cosmos_client = QueryClient::new(
                                        chain_config.clone().to_chain_config(),
                                        None,
                                    )
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

                        if *event_hash == IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH {
                            // 3. Decode the event data
                            match IWavsServiceManager::ServiceURIUpdated::decode_log_data(
                                log.data(),
                            ) {
                                Ok(decoded_event) => {
                                    let service_uri: String = decoded_event.serviceURI;
                                    // check if this is a service we're interested in
                                    if let Some(service_id) = self
                                        .lookup_maps
                                        .service_manager
                                        .read()
                                        .unwrap()
                                        .get_by_right(&contract_address.into())
                                    {
                                        dispatcher_commands.push(
                                            DispatcherCommand::ChangeServiceUri {
                                                service_id: service_id.clone(),
                                                uri: service_uri,
                                            },
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to decode ServiceURIUpdated data: {}",
                                        e
                                    );
                                }
                            }
                        }

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
                            for trigger_config in self.lookup_maps.get_trigger_configs(lookup_ids) {
                                dispatcher_commands.push(DispatcherCommand::Trigger(
                                    TriggerAction {
                                        data: TriggerData::EvmContractEvent {
                                            contract_address,
                                            chain_name: chain_name.clone(),
                                            log: log.inner.data.clone(),
                                            block_height,
                                        },
                                        config: trigger_config.clone(),
                                    },
                                ));
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
                        let update_event = contract_events
                            .iter()
                            .find_map(|(address, event)| {
                                wavs_types::contracts::cosmwasm::service_manager::event::WavsServiceUriUpdatedEvent::try_from(event).ok()
                                    .map(|event| (address.clone(), event))
                            });

                        if let Some((address, event)) = update_event {
                            // check if this is a service we're interested in
                            if let Some(service_id) = self
                                .lookup_maps
                                .service_manager
                                .read()
                                .unwrap()
                                .get_by_right(&address)
                            {
                                dispatcher_commands.push(DispatcherCommand::ChangeServiceUri {
                                    service_id: service_id.clone(),
                                    uri: event.service_uri,
                                });
                            }
                        }

                        let triggers_by_contract_event_lock = self
                            .lookup_maps
                            .triggers_by_cosmos_contract_event
                            .read()
                            .unwrap();

                        for (contract_address, event) in contract_events {
                            if let Some(lookup_ids) = triggers_by_contract_event_lock.get(&(
                                chain_name.clone(),
                                contract_address.clone(),
                                event.ty.clone(),
                            )) {
                                for trigger_config in
                                    self.lookup_maps.get_trigger_configs(lookup_ids)
                                {
                                    dispatcher_commands.push(DispatcherCommand::Trigger(
                                        TriggerAction {
                                            data: TriggerData::CosmosContractEvent {
                                                contract_address: contract_address.clone(),
                                                chain_name: chain_name.clone(),
                                                event: event.clone(),
                                                block_height,
                                            },
                                            config: trigger_config.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                    }

                    // process block-based triggers
                    dispatcher_commands.extend(self.process_blocks(chain_name, block_height));
                }
                StreamTriggers::EvmBlock {
                    chain_name,
                    block_height,
                } => {
                    dispatcher_commands.extend(self.process_blocks(chain_name, block_height));
                }
                StreamTriggers::Cron {
                    trigger_time,
                    lookup_ids,
                } => {
                    for trigger_config in self.lookup_maps.get_trigger_configs(&lookup_ids) {
                        dispatcher_commands.push(DispatcherCommand::Trigger(TriggerAction {
                            data: TriggerData::Cron { trigger_time },
                            config: trigger_config.clone(),
                        }));
                    }
                }
            }

            if !dispatcher_commands.is_empty() {
                tracing::info!(
                    "Sending {} commands to dispatcher",
                    dispatcher_commands.len()
                );
                for (idx, command) in dispatcher_commands.iter().enumerate() {
                    if let DispatcherCommand::Trigger(action) = command {
                        // Log the trigger action details
                        tracing::debug!(
                            "Trigger action (in this batch) {}: service_id={}, workflow_id={}, trigger_data={:?}",
                            idx + 1,
                            action.config.service_id,
                            action.config.workflow_id,
                            action.data
                        );
                    }
                }

                self.send_dispatcher_commands(dispatcher_commands).await?;
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        Ok(())
    }

    /// Process blocks and return trigger actions for any triggers that should fire
    pub fn process_blocks(
        &self,
        chain_name: ChainName,
        block_height: u64,
    ) -> Vec<DispatcherCommand> {
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
            self.lookup_maps
                .get_trigger_configs(&firing_lookup_ids)
                .into_iter()
                .map(|trigger_config| {
                    DispatcherCommand::Trigger(TriggerAction {
                        data: TriggerData::BlockInterval {
                            chain_name: chain_name.clone(),
                            block_height: block_height.get(),
                        },
                        config: trigger_config,
                    })
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    #[cfg(debug_assertions)]
    pub fn get_lookup_maps(&self) -> &Arc<LookupMaps> {
        &self.lookup_maps
    }
}

#[cfg(test)]
mod tests {}
