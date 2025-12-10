pub mod error;
pub mod lookup;
pub mod schedulers;
pub mod streams;

use crate::{
    config::Config,
    dispatcher::DispatcherCommand,
    services::Services,
    subsystems::trigger::streams::{
        cosmos_stream::StreamTriggerCosmosContractEvent,
        evm_stream::client::{EvmTriggerStreams, EvmTriggerStreamsController},
        local_command_stream,
    },
    tracing_service_info, AppContext,
};
use alloy_sol_types::SolEvent;
use anyhow::Result;
use error::TriggerError;
use futures::{stream::SelectAll, StreamExt};
use iri_string::types::UriString;
use layer_climb::prelude::*;
use lookup::{LookupId, LookupMaps};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::Arc,
};
use streams::{cosmos_stream, cron_stream, evm_stream, MultiplexedStream, StreamTriggers};
use tracing::instrument;
use utils::telemetry::TriggerMetrics;
use wavs_types::{
    contracts::cosmwasm::service_manager::event::WavsServiceUriUpdatedEvent, AnyChainConfig,
    ByteArray, ChainConfigs, ChainKey, IWavsServiceManager, ServiceId, Trigger, TriggerAction,
    TriggerConfig, TriggerData,
};

#[derive(Debug)]
pub enum TriggerCommand {
    Kill,
    StartListeningChain {
        chain: ChainKey,
    },
    StartListeningCron,
    WatchEvmBlocks {
        chain: ChainKey,
    },
    WatchEvmContractEvents {
        chain: ChainKey,
        addresses: Vec<alloy_primitives::Address>,
        event_hashes: Vec<alloy_primitives::B256>,
    },
    StartListeningAtProto,
    ManualTrigger(Box<TriggerAction>),
}

impl TriggerCommand {
    pub fn map(trigger_config: &TriggerConfig, chain_configs: &ChainConfigs) -> Vec<Self> {
        match &trigger_config.trigger {
            Trigger::Cron { .. } => vec![Self::StartListeningCron],
            Trigger::EvmContractEvent {
                chain,
                address,
                event_hash,
            } => {
                vec![
                    Self::StartListeningChain {
                        chain: chain.clone(),
                    },
                    Self::WatchEvmContractEvents {
                        chain: chain.clone(),
                        addresses: vec![*address],
                        event_hashes: vec![event_hash.into_inner().into()],
                    },
                ]
            }
            Trigger::CosmosContractEvent { chain, .. } => {
                vec![Self::StartListeningChain {
                    chain: chain.clone(),
                }]
            }
            Trigger::BlockInterval { chain, .. } => match chain_configs.get_chain(chain) {
                Some(chain_config) => match chain_config {
                    AnyChainConfig::Evm(_) => {
                        vec![
                            Self::StartListeningChain {
                                chain: chain.clone(),
                            },
                            Self::WatchEvmBlocks {
                                chain: chain.clone(),
                            },
                        ]
                    }
                    AnyChainConfig::Cosmos(_) => {
                        vec![Self::StartListeningChain {
                            chain: chain.clone(),
                        }]
                    }
                },
                None => {
                    tracing::warn!("Block interval set for non-existant chain-config: {chain}");
                    Vec::new()
                }
            },
            Trigger::AtProtoEvent { .. } => {
                vec![Self::StartListeningAtProto]
            }
            Trigger::Manual => Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct TriggerManager {
    pub chain_configs: Arc<std::sync::RwLock<ChainConfigs>>,
    pub command_sender: tokio::sync::mpsc::UnboundedSender<TriggerCommand>,
    trigger_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    command_receiver:
        Arc<std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<TriggerCommand>>>>,
    lookup_maps: Arc<LookupMaps>,
    metrics: TriggerMetrics,
    #[cfg(feature = "dev")]
    pub disable_networking: bool,
    pub services: Services,
    pub evm_controllers: Arc<std::sync::RwLock<HashMap<ChainKey, EvmTriggerStreamsController>>>,
    pub config: Config,
}

impl TriggerManager {
    #[allow(clippy::new_without_default)]
    #[instrument(skip(services), fields(subsys = "TriggerManager"))]
    pub fn new(
        config: &Config,
        metrics: TriggerMetrics,
        services: Services,
        trigger_to_dispatcher_tx: crossbeam::channel::Sender<DispatcherCommand>,
    ) -> Result<Self, TriggerError> {
        let (command_sender, command_receiver) = tokio::sync::mpsc::unbounded_channel();

        Ok(Self {
            chain_configs: config.chains.clone(),
            lookup_maps: Arc::new(LookupMaps::new(services.clone(), metrics.clone())),
            trigger_to_dispatcher_tx,
            command_sender,
            command_receiver: Arc::new(std::sync::Mutex::new(Some(command_receiver))),
            metrics,
            #[cfg(feature = "dev")]
            disable_networking: config.disable_trigger_networking,
            services,
            evm_controllers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            config: config.clone(),
        })
    }

    #[instrument(skip(self), fields(subsys = "TriggerManager"))]
    pub fn add_service(&self, service: &wavs_types::Service) -> Result<(), TriggerError> {
        // The mechanics of adding a trigger are that we:

        // 1. Setup all the records needed to track the trigger in various "lookup" maps.
        // 2a. If the trigger needs some kind of stream to kick it off, we need to create that stream.
        // 2b. Actual stream-creation happens by way of a "local command" so that everything is handled in `start_watcher` (helps with lifetime issues).
        //
        // It doesn't really matter what order the multiplexed streams are polled in, a trigger simply
        // will not be fired until the stream that kicks it off is polled (i.e. this definitively happens _after_ the stream is created).

        self.lookup_maps.add_service(service)?;

        // Ensure the service manager's chain is being listened to for service change events
        // This is needed even if the service has no workflows, so service URI changes can be detected
        self.command_sender
            .send(TriggerCommand::StartListeningChain {
                chain: service.manager.chain().clone(),
            })?;

        match service.manager.clone() {
            wavs_types::ServiceManager::Evm { chain, address } => {
                self.command_sender
                    .send(TriggerCommand::WatchEvmContractEvents {
                        chain,
                        addresses: vec![address],
                        event_hashes: vec![IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH],
                    })?;
            }
            wavs_types::ServiceManager::Cosmos { .. } => {
                /* Nothing to do, Cosmos consumes all events, service URI changes will be handled */
            }
        }

        let chain_configs = self.chain_configs.read().unwrap().clone();

        for (id, workflow) in &service.workflows {
            let config = TriggerConfig {
                service_id: service.id(),
                workflow_id: id.clone(),
                trigger: workflow.trigger.clone(),
            };

            for command in TriggerCommand::map(&config, &chain_configs) {
                self.command_sender.send(command)?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "TriggerManager"))]
    pub fn remove_service(&self, service_id: ServiceId) -> Result<(), TriggerError> {
        self.lookup_maps.remove_service(service_id.clone())?;

        // TODO - consider sending commands to:
        // 1. stop listening to chains if no triggers remain for them
        // 2. remove any cron jobs if no triggers remain for them
        // 3. remove any EVM log subscriptions if no triggers remain for them
        // 4. remove any block subscriptions if no triggers remain for them

        Ok(())
    }

    #[instrument(skip(self, ctx), fields(subsys = "TriggerManager"))]
    pub fn start(&self, ctx: AppContext) {
        ctx.rt.block_on(self.start_watcher()).unwrap();
    }

    pub fn send_dispatcher_commands(
        &self,
        commands: impl IntoIterator<Item = DispatcherCommand>,
    ) -> Result<(), TriggerError> {
        for command in commands {
            match &command {
                DispatcherCommand::Trigger(action) => {
                    #[cfg(feature = "dev")]
                    if std::env::var("WAVS_FORCE_TRIGGER_ERROR_XXX").is_ok() {
                        self.metrics.increment_total_errors("forced trigger error");
                        continue;
                    }

                    tracing_service_info!(
                        &self.services,
                        action.config.service_id,
                        "Sending trigger action for workflow {}",
                        action.config.workflow_id,
                    );

                    self.metrics
                        .record_trigger_fired(action.data.chain(), action.data.trigger_type());
                }
                DispatcherCommand::ChangeServiceUri { service_id, uri } => {
                    tracing_service_info!(
                        &self.services,
                        service_id,
                        "Changing service URI to {}",
                        uri
                    );
                }
            }

            let start = std::time::Instant::now();
            self.trigger_to_dispatcher_tx
                .send(command)
                .map_err(Box::new)?;

            self.metrics
                .record_trigger_sent_dispatcher_command(start.elapsed().as_secs_f64());
        }

        Ok(())
    }

    pub fn add_trigger(&self, trigger: TriggerAction) -> Result<(), TriggerError> {
        self.command_sender
            .send(TriggerCommand::ManualTrigger(Box::new(trigger)))?;
        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "TriggerManager"))]
    async fn start_watcher(&self) -> Result<(), TriggerError> {
        let mut multiplexed_stream: MultiplexedStream = SelectAll::new();

        let local_command_stream = local_command_stream::start_local_command_stream(
            self.command_receiver.lock().unwrap().take().unwrap(),
            self.metrics.clone(),
        )?;
        multiplexed_stream.push(local_command_stream);

        let mut cosmos_clients = HashMap::new();

        let mut listening_chains = HashSet::new();
        let mut has_started_cron_stream = false;
        let mut has_started_atproto_stream = false;

        // Create a stream for cron triggers that produces a trigger for each due task

        while let Some(res) = multiplexed_stream.next().await {
            let res = match res {
                Err(err) => {
                    tracing::error!("{:?}", err);
                    continue;
                }
                Ok(res) => res,
            };

            tracing::debug!("Processing trigger stream event: {:?}", res);
            let mut dispatcher_commands = Vec::new();

            match res {
                StreamTriggers::LocalCommand(command) => {
                    match command {
                        TriggerCommand::Kill => {
                            tracing::info!("Received kill command, shutting down trigger manager");
                            break;
                        }
                        TriggerCommand::ManualTrigger(trigger_action) => {
                            // send it directly to dispatcher
                            dispatcher_commands.push(DispatcherCommand::Trigger(*trigger_action));
                        }
                        TriggerCommand::StartListeningCron => {
                            #[cfg(feature = "dev")]
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
                        TriggerCommand::StartListeningChain { chain } => {
                            #[cfg(feature = "dev")]
                            if self.disable_networking {
                                tracing::warn!(
                                    "Networking is disabled, skipping chain stream start"
                                );
                                continue;
                            }
                            if listening_chains.contains(&chain) {
                                tracing::debug!("Already listening to chain {chain}");
                                continue;
                            }

                            // insert right away, before we get to an await point
                            listening_chains.insert(chain.clone());

                            let chain_config =
                                match self.chain_configs.read().unwrap().get_chain(&chain) {
                                    Some(config) => config,
                                    None => {
                                        tracing::error!("No chain config found for {chain}");
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

                                    cosmos_clients.insert(chain.clone(), cosmos_client.clone());

                                    // Start the Cosmos event stream
                                    match cosmos_stream::start_cosmos_stream(
                                        cosmos_client.clone(),
                                        chain.clone(),
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
                                    if chain_config.ws_endpoints.is_empty() {
                                        return Err(TriggerError::EvmMissingWebsocket(
                                            chain.clone(),
                                        ));
                                    }

                                    let chain_key: ChainKey = (&chain_config).into();
                                    let EvmTriggerStreams {
                                        controller,
                                        block_height_stream,
                                        log_stream,
                                        // ignoring this for now
                                        new_pending_transaction_stream: _,
                                    } = EvmTriggerStreams::new(
                                        chain_config.ws_endpoints,
                                        chain_key,
                                        chain_config.ws_priority_endpoint_index,
                                    );

                                    self.evm_controllers
                                        .write()
                                        .unwrap()
                                        .insert(chain.clone(), controller);

                                    // Start the EVM event stream
                                    // however, the actual subscription for log filters is set via the controller
                                    match evm_stream::start_evm_event_stream(
                                        chain.clone(),
                                        log_stream,
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
                                    // however, the actual subscription for blocks is gated via the controller
                                    match evm_stream::start_evm_block_stream(
                                        chain.clone(),
                                        block_height_stream,
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
                        TriggerCommand::WatchEvmContractEvents {
                            chain,
                            addresses,
                            event_hashes,
                        } => match self.evm_controllers.read().unwrap().get(&chain) {
                            Some(evm_controller) => {
                                evm_controller
                                    .subscriptions
                                    .enable_logs(addresses, event_hashes);
                            }
                            None => {
                                tracing::error!(
                                        "No EVM controller found for chain {chain}, cannot watch contract event"
                                    );
                                continue;
                            }
                        },
                        TriggerCommand::WatchEvmBlocks { chain } => {
                            match self.evm_controllers.read().unwrap().get(&chain) {
                                Some(evm_controller) => {
                                    evm_controller.subscriptions.toggle_block_height(true);
                                }
                                None => {
                                    tracing::error!(
                                        "No EVM controller found for chain {chain}, cannot watch blocks"
                                    );
                                    continue;
                                }
                            }
                        }
                        TriggerCommand::StartListeningAtProto => {
                            #[cfg(feature = "dev")]
                            if self.disable_networking {
                                tracing::warn!(
                                    "Networking is disabled, skipping ATProto stream start"
                                );
                                continue;
                            }

                            if has_started_atproto_stream {
                                tracing::debug!("ATProto stream already started, skipping");
                                continue;
                            }

                            has_started_atproto_stream = true;

                            // Subscribe to all ATProto events - filtering will be done in the lookup system
                            let jetstream_config = streams::atproto_jetstream::JetstreamConfig {
                                endpoint: self.config.jetstream_endpoint.clone(),
                                wanted_collections: vec![], // Empty means subscribe to all collections
                                wanted_dids: None,          // Listen to all repos
                                cursor: None,
                                compression: false,
                                max_message_size: self.config.jetstream_max_message_size,
                                require_hello: false,
                            };

                            // Start the ATProto Jetstream stream
                            match streams::atproto_jetstream::start_jetstream_stream(
                                jetstream_config,
                                self.metrics.clone(),
                            )
                            .await
                            {
                                Ok(atproto_stream) => {
                                    multiplexed_stream.push(atproto_stream);
                                    tracing::info!("Started ATProto Jetstream stream");
                                }
                                Err(err) => {
                                    tracing::error!(
                                        "Failed to start ATProto Jetstream stream: {:?}",
                                        err
                                    );
                                    continue;
                                }
                            }
                        }
                    }
                }
                StreamTriggers::Evm {
                    log,
                    chain,
                    block_number,
                    tx_hash,
                    log_index,
                    block_hash,
                    tx_index,
                    block_timestamp,
                } => {
                    if let Some(event_hash) = log.topic0() {
                        let contract_address = log.address();

                        if *event_hash == IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH {
                            // 3. Decode the event data
                            match IWavsServiceManager::ServiceURIUpdated::decode_log_data(
                                log.data(),
                            ) {
                                Ok(decoded_event) => {
                                    let service_uri =
                                        UriString::try_from(decoded_event.serviceURI)?;
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
                            chain.clone(),
                            contract_address,
                            ByteArray::new(**event_hash),
                        )) {
                            let trigger_data = TriggerData::EvmContractEvent {
                                contract_address,
                                chain,
                                log_data: log.data().clone(),
                                tx_hash,
                                block_number,
                                log_index,
                                block_hash,
                                block_timestamp,
                                tx_index,
                            };

                            for trigger_config in self.lookup_maps.get_trigger_configs(lookup_ids) {
                                dispatcher_commands.push(DispatcherCommand::Trigger(
                                    TriggerAction {
                                        data: trigger_data.clone(),
                                        config: trigger_config.clone(),
                                    },
                                ));
                            }
                        }
                    }
                }
                StreamTriggers::Cosmos {
                    contract_events,
                    chain,
                    block_height,
                } => {
                    // extra scope in order to properly drop the locks
                    {
                        let triggers_by_contract_event_lock = self
                            .lookup_maps
                            .triggers_by_cosmos_contract_event
                            .read()
                            .unwrap();

                        for StreamTriggerCosmosContractEvent {
                            contract_address,
                            event,
                            event_index,
                        } in contract_events
                        {
                            if layer_climb::events::Event::from(&event)
                                .is_type(WavsServiceUriUpdatedEvent::EVENT_TYPE)
                            {
                                let service_uri = event.attributes.iter().find_map(|attr| {
                                    if attr.key
                                        == WavsServiceUriUpdatedEvent::EVENT_ATTR_KEY_SERVICE_URI
                                    {
                                        UriString::try_from(attr.value.clone()).ok()
                                    } else {
                                        None
                                    }
                                });

                                let service_uri = match service_uri {
                                    Some(uri) => uri,
                                    None => {
                                        tracing::error!(
                                            "ServiceURIUpdated event missing {} attribute",
                                            WavsServiceUriUpdatedEvent::EVENT_ATTR_KEY_SERVICE_URI
                                        );
                                        continue;
                                    }
                                };

                                if let Some(service_id) = self
                                    .lookup_maps
                                    .service_manager
                                    .read()
                                    .unwrap()
                                    .get_by_right(&contract_address.clone().into())
                                {
                                    dispatcher_commands.push(DispatcherCommand::ChangeServiceUri {
                                        service_id: service_id.clone(),
                                        uri: service_uri,
                                    });
                                }
                            }
                            if let Some(lookup_ids) = triggers_by_contract_event_lock.get(&(
                                chain.clone(),
                                contract_address.clone(),
                                event.ty.clone(),
                            )) {
                                let trigger_data = TriggerData::CosmosContractEvent {
                                    contract_address,
                                    chain: chain.clone(),
                                    event,
                                    event_index,
                                    block_height,
                                };
                                for trigger_config in
                                    self.lookup_maps.get_trigger_configs(lookup_ids)
                                {
                                    dispatcher_commands.push(DispatcherCommand::Trigger(
                                        TriggerAction {
                                            data: trigger_data.clone(),
                                            config: trigger_config.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                    }

                    // process block-based triggers
                    dispatcher_commands.extend(self.process_blocks(chain, block_height));
                }
                StreamTriggers::EvmBlock {
                    chain,
                    block_height,
                } => {
                    dispatcher_commands.extend(self.process_blocks(chain, block_height));
                }
                StreamTriggers::Cron { hits } => {
                    // Process each cron hit (group of triggers at the same scheduled time)
                    for hit in hits {
                        for lookup_id in &hit.lookup_ids {
                            if let Some(trigger_config) =
                                self.lookup_maps.get_trigger_config(*lookup_id)
                            {
                                dispatcher_commands.push(DispatcherCommand::Trigger(
                                    TriggerAction {
                                        data: TriggerData::Cron {
                                            trigger_time: hit.scheduled_time,
                                        },
                                        config: trigger_config.clone(),
                                    },
                                ));
                            }
                        }
                    }
                }
                StreamTriggers::AtProto { event } => {
                    let action_enum = event.action.clone();

                    // Find matching triggers using multiple lookup strategies
                    let mut matched_lookup_ids: HashSet<LookupId> = HashSet::new();

                    // Strategy 1: Exact match (collection, repo, action)
                    {
                        let triggers_by_atproto_lock =
                            self.lookup_maps.triggers_by_atproto_event.read().unwrap();

                        // Check exact collection/repo/action match
                        if let Some(lookup_ids) = triggers_by_atproto_lock.get(&(
                            event.collection.clone(),
                            Some(event.repo.clone()),
                            Some(action_enum.clone()),
                        )) {
                            matched_lookup_ids.extend(lookup_ids);
                        }

                        // Check collection/repo match (any action)
                        if let Some(lookup_ids) = triggers_by_atproto_lock.get(&(
                            event.collection.clone(),
                            Some(event.repo.clone()),
                            None,
                        )) {
                            matched_lookup_ids.extend(lookup_ids);
                        }

                        // Check collection/action match (any repo)
                        if let Some(lookup_ids) = triggers_by_atproto_lock.get(&(
                            event.collection.clone(),
                            None,
                            Some(action_enum.clone()),
                        )) {
                            matched_lookup_ids.extend(lookup_ids);
                        }

                        // Check collection match (any repo, any action)
                        if let Some(lookup_ids) =
                            triggers_by_atproto_lock.get(&(event.collection.clone(), None, None))
                        {
                            matched_lookup_ids.extend(lookup_ids);
                        }
                    }

                    // Strategy 2: Pattern matching for collections with wildcards
                    {
                        let triggers_by_atproto_lock =
                            self.lookup_maps.triggers_by_atproto_event.read().unwrap();

                        for ((collection_pattern, repo_did_filter, action_filter), lookup_ids) in
                            triggers_by_atproto_lock.iter()
                        {
                            // Skip if already matched by exact lookup
                            if matched_lookup_ids.intersection(lookup_ids).next().is_some() {
                                continue;
                            }

                            // Check collection pattern match (supports wildcards)
                            if self
                                .matches_collection_pattern(collection_pattern, &event.collection)
                            {
                                // Check repo filter
                                let repo_matches = match repo_did_filter {
                                    Some(filter_did) => filter_did == &event.repo,
                                    None => true, // any repo
                                };

                                // Check action filter
                                let action_matches = match action_filter {
                                    Some(filter_action) => filter_action == &action_enum,
                                    None => true, // any action
                                };

                                if repo_matches && action_matches {
                                    matched_lookup_ids.extend(lookup_ids);
                                }
                            }
                        }
                    }

                    // Create trigger actions for all matched lookups
                    if !matched_lookup_ids.is_empty() {
                        let trigger_data = TriggerData::AtProtoEvent {
                            sequence: event.sequence,
                            timestamp: event.timestamp,
                            repo: event.repo.clone(),
                            collection: event.collection.clone(),
                            rkey: event.rkey.clone(),
                            action: action_enum.clone(),
                            cid: event.cid.clone(),
                            record: event.record.clone(),
                            rev: event.rev.clone(),
                            op_index: event.op_index,
                        };

                        for trigger_config in
                            self.lookup_maps.get_trigger_configs(&matched_lookup_ids)
                        {
                            dispatcher_commands.push(DispatcherCommand::Trigger(TriggerAction {
                                data: trigger_data.clone(),
                                config: trigger_config.clone(),
                            }));
                        }

                        tracing::info!(
                            "ATProto event matched {} triggers: collection={}, repo={}, action={}",
                            matched_lookup_ids.len(),
                            event.collection,
                            event.repo,
                            action_enum
                        );
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
                        tracing::debug!(
                            batch = idx + 1,
                            service_id = %action.config.service_id,
                            workflow_id = %action.config.workflow_id,
                            trigger_data = ?action.data,
                            "Trigger action (in this batch)"
                        );
                    }
                }

                self.send_dispatcher_commands(dispatcher_commands)?;
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

        // just a little cleanup to avoid errors in e2e tests
        for (_, controller) in self.evm_controllers.write().unwrap().drain() {
            drop(controller);
        }

        Ok(())
    }

    /// Process blocks and return trigger actions for any triggers that should fire
    pub fn process_blocks(&self, chain: ChainKey, block_height: u64) -> Vec<DispatcherCommand> {
        let block_height = match NonZeroU64::new(block_height) {
            Some(height) => height,
            None => {
                self.metrics.increment_total_errors("block height is zero");
                return Vec::new();
            }
        };
        // Get the triggers that should fire at this block height
        let firing_lookup_ids: Vec<LookupId> =
            match self.lookup_maps.block_schedulers.get_mut(&chain) {
                Some(mut scheduler) => scheduler
                    .tick(block_height.into())
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect(),
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
                            chain: chain.clone(),
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

    /// Check if a collection pattern matches the actual collection
    /// Supports wildcard patterns with '*' (e.g., "app.bsky.feed.*")
    fn matches_collection_pattern(&self, pattern: &str, actual: &str) -> bool {
        // Exact match
        if pattern == actual {
            return true;
        }

        // Wildcard pattern matching
        if let Some(prefix) = pattern.strip_suffix(".*") {
            return actual.starts_with(prefix)
                && actual.len() > prefix.len()
                && actual[prefix.len()..].starts_with('.');
        }

        false
    }

    #[cfg(feature = "dev")]
    pub fn get_lookup_maps(&self) -> &Arc<LookupMaps> {
        &self.lookup_maps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::{config::Config, services::Services};
    use utils::{
        storage::db::WavsDb, telemetry::TriggerMetrics, test_utils::address::rand_address_evm,
    };
    use wavs_types::{
        Component, ComponentDigest, ComponentSource, ServiceManager, SignatureKind, Submit,
        Trigger, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowId,
    };

    #[test]
    fn test_add_trigger() {
        let config = Config::default();

        let db_storage = WavsDb::new().unwrap();
        let services = Services::new(db_storage);

        let metrics = TriggerMetrics::new(opentelemetry::global::meter("test"));
        let (dispatcher_tx, dispatcher_rx) = crossbeam::channel::unbounded::<DispatcherCommand>();

        let service = wavs_types::Service {
            name: "serv1".to_string(),
            status: wavs_types::ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain: "evm:anvil".parse().unwrap(),
                address: rand_address_evm(),
            },
            workflows: vec![(
                "workflow-1".parse().unwrap(),
                Workflow {
                    trigger: Trigger::Manual,
                    component: Component::new(ComponentSource::Digest(ComponentDigest::hash(
                        [0; 32],
                    ))),
                    submit: Submit::Aggregator {
                        url: "http://example.com".to_string(),
                        component: Box::new(Component::new(ComponentSource::Digest(
                            ComponentDigest::hash([0; 32]),
                        ))),
                        signature_kind: SignatureKind::evm_default(),
                    },
                },
            )]
            .into_iter()
            .collect(),
        };
        services.save(&service).unwrap();

        let trigger_manager =
            TriggerManager::new(&config, metrics, services, dispatcher_tx).unwrap();

        let ctx = utils::context::AppContext::new();
        std::thread::spawn({
            let trigger_manager = trigger_manager.clone();
            let ctx = ctx.clone();
            move || {
                trigger_manager.start(ctx);
            }
        });

        // short sleep for trigger manager to kick in
        std::thread::sleep(Duration::from_millis(100));

        for i in 0..6 {
            let action = TriggerAction {
                config: TriggerConfig {
                    service_id: service.id(),
                    workflow_id: WorkflowId::new("workflow-1").unwrap(),
                    trigger: Trigger::Manual,
                },
                data: TriggerData::Raw(vec![i as u8]),
            };

            let result = trigger_manager.add_trigger(action);
            assert!(result.is_ok(), "Failed to add trigger {}: {:?}", i, result);
        }

        let mut received_count = 0;
        while let Ok(command) = dispatcher_rx.recv() {
            if let DispatcherCommand::Trigger(action) = command {
                if let TriggerData::Raw(data) = &action.data {
                    assert_eq!(
                        data,
                        &vec![received_count as u8],
                        "Trigger {} data mismatch",
                        received_count
                    );
                    received_count += 1;
                    if received_count == 6 {
                        break;
                    }
                }
            }
        }
        assert_eq!(received_count, 6, "Expected to receive 6 triggers");

        ctx.kill();
    }
}
