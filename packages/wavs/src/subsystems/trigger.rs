pub mod error;
pub mod lookup;
pub mod recovery;
pub mod schedulers;
pub mod streams;

use crate::{
    config::Config,
    dispatcher::DispatcherCommand,
    services::Services,
    subsystems::trigger::streams::{
        cosmos_stream::StreamTriggerCosmosContractEvent, local_command_stream,
    },
    tracing_service_info, AppContext,
};
use alloy_provider::Provider;
use alloy_sol_types::SolEvent;
use anyhow::Result;
use error::TriggerError;
use futures::{stream::SelectAll, StreamExt};
use layer_climb::prelude::*;
use lookup::LookupMaps;
use std::pin::Pin;
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::Arc,
};
use streams::{catchup, cosmos_stream, cron_stream, evm_stream, MultiplexedStream, StreamTriggers};
use tokio_stream::StreamMap;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use utils::{
    config::{AnyChainConfig, ChainConfigs, EvmChainConfigExt},
    evm_client::EvmQueryClient,
    telemetry::TriggerMetrics,
};
use wavs_types::{
    ByteArray, ChainKey, EventId, IWavsServiceManager, ServiceId, ServiceManager, Trigger,
    TriggerAction, TriggerConfig, TriggerData,
};

#[derive(Debug)]
pub enum TriggerCommand {
    Kill,
    StartListeningChain { chain: ChainKey },
    ResubscribeEvmChain { chain: ChainKey },
    StartListeningCron,
    StartCatchup { chain: ChainKey, from_block: u64 },
    CatchupCompleted { chain: ChainKey },
    ManualTrigger(Box<TriggerAction>),
}

impl TriggerCommand {
    pub fn new(trigger_config: &TriggerConfig) -> Option<Self> {
        match &trigger_config.trigger {
            Trigger::Cron { .. } => Some(Self::StartListeningCron),
            Trigger::EvmContractEvent { chain, .. }
            | Trigger::CosmosContractEvent { chain, .. }
            | Trigger::BlockInterval { chain, .. } => Some(Self::StartListeningChain {
                chain: chain.clone(),
            }),
            Trigger::Manual => None,
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
    recovery_manager: Arc<recovery::RecoveryManager>,
    metrics: TriggerMetrics,
    #[cfg(feature = "dev")]
    pub disable_networking: bool,
    pub services: Services,
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

        // Create recovery manager with shorter delay before starting recovery
        let recovery_manager = Arc::new(recovery::RecoveryManager::new(
            std::time::Duration::from_secs(3),
        ));

        Ok(Self {
            chain_configs: Arc::new(std::sync::RwLock::new(config.chains.clone())),
            lookup_maps: Arc::new(LookupMaps::new(services.clone(), metrics.clone())),
            recovery_manager,
            trigger_to_dispatcher_tx,
            command_sender,
            command_receiver: Arc::new(std::sync::Mutex::new(Some(command_receiver))),
            metrics,
            #[cfg(feature = "dev")]
            disable_networking: config.disable_trigger_networking,
            services,
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
        let manager_chain = service.manager.chain().clone();
        self.command_sender
            .send(TriggerCommand::StartListeningChain {
                chain: manager_chain.clone(),
            })?;

        // Accumulate chains that need resubscribe (EVM)
        let mut evm_chains_to_refresh: std::collections::HashSet<ChainKey> =
            std::collections::HashSet::new();

        for (id, workflow) in &service.workflows {
            let config = TriggerConfig {
                service_id: service.id(),
                workflow_id: id.clone(),
                trigger: workflow.trigger.clone(),
            };

            if let Some(command) = TriggerCommand::new(&config) {
                self.command_sender.send(command)?;
            }

            if let Trigger::EvmContractEvent { chain, .. } = &workflow.trigger {
                evm_chains_to_refresh.insert(chain.clone());
            }
        }

        // Ensure event stream filters include this service’s contracts/events
        evm_chains_to_refresh.insert(manager_chain);
        for chain in evm_chains_to_refresh {
            let _ = self
                .command_sender
                .send(TriggerCommand::ResubscribeEvmChain { chain });
        }

        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "TriggerManager"))]
    pub fn remove_service(&self, service_id: ServiceId) -> Result<(), TriggerError> {
        // determine affected chains before removing
        let mut evm_chains_to_refresh: std::collections::HashSet<ChainKey> =
            std::collections::HashSet::new();
        if let Ok(service) = self.services.get(&service_id) {
            evm_chains_to_refresh.insert(service.manager.chain().clone());
            for workflow in service.workflows.values() {
                if let Trigger::EvmContractEvent { chain, .. } = &workflow.trigger {
                    evm_chains_to_refresh.insert(chain.clone());
                }
            }
        }

        self.lookup_maps.remove_service(service_id.clone())?;

        for chain in evm_chains_to_refresh {
            let _ = self
                .command_sender
                .send(TriggerCommand::ResubscribeEvmChain { chain });
        }
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
        let mut evm_clients = HashMap::new();
        // EVM stream management via keyed StreamMap and cancellation tokens
        let mut evm_event_streams: StreamMap<
            ChainKey,
            Pin<
                Box<
                    dyn futures::Stream<Item = std::result::Result<StreamTriggers, TriggerError>>
                        + Send,
                >,
            >,
        > = StreamMap::new();
        let mut evm_block_streams: StreamMap<
            ChainKey,
            Pin<
                Box<
                    dyn futures::Stream<Item = std::result::Result<StreamTriggers, TriggerError>>
                        + Send,
                >,
            >,
        > = StreamMap::new();
        let mut evm_event_cancels: HashMap<ChainKey, CancellationToken> = HashMap::new();

        let mut listening_chains = HashSet::new();
        // Track active catchup streams per chain to avoid duplicates
        let mut active_catchups: HashSet<ChainKey> = HashSet::new();
        let mut has_started_cron_stream = false;

        // Create a stream for cron triggers that produces a trigger for each due task

        // Monitor recovery chains periodically
        let mut recovery_check_interval =
            tokio::time::interval(tokio::time::Duration::from_secs(10));

        loop {
            let maybe_item = tokio::select! {
                _ = recovery_check_interval.tick() => {
                    // Check if any chains need recovery
                    for chain in self.recovery_manager.get_all_recovery_chains().await {
                        if let Some(recovery_block) = self.recovery_manager.needs_recovery(&chain).await {
                            if active_catchups.contains(&chain) {
                                continue;
                            }
                            tracing::info!("Chain {} needs recovery from block {}", chain, recovery_block);
                            let _ = self.command_sender.send(TriggerCommand::StartCatchup {
                                chain: chain.clone(),
                                from_block: recovery_block
                            });
                        }
                    }
                    continue;
                }
                // First, EVM event streams
                r1 = evm_event_streams.next() => {
                    r1.map(|(_k, item)| item)
                }
                // Then, EVM block streams
                r2 = evm_block_streams.next() => {
                    r2.map(|(_k, item)| item)
                }
                // Finally, the general multiplexed streams
                res = multiplexed_stream.next() => res,
            };

            let Some(res) = maybe_item else {
                // Some branch yielded no item (e.g., an empty StreamMap). Yield to avoid busy loop.
                tokio::task::yield_now().await;
                continue;
            };

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
                        TriggerCommand::StartCatchup { chain, from_block } => {
                            #[cfg(feature = "dev")]
                            if self.disable_networking {
                                tracing::warn!(
                                    "Networking is disabled, skipping catchup stream start"
                                );
                                continue;
                            }

                            // Avoid starting duplicate catchup streams for the same chain
                            if active_catchups.contains(&chain) {
                                tracing::debug!(
                                    "Catchup already active for chain {}, skipping",
                                    chain
                                );
                                continue;
                            }

                            // Create a temporary EVM client for catchup
                            if let Some(AnyChainConfig::Evm(chain_config)) =
                                self.chain_configs.read().unwrap().get_chain(&chain)
                            {
                                let endpoint = chain_config
                                    .query_client_endpoint()
                                    .map_err(|e| TriggerError::EvmClient(chain.clone(), e));
                                match endpoint {
                                    Ok(endpoint) => match EvmQueryClient::new(endpoint).await {
                                        Ok(catchup_client) => {
                                            // Start immediate block-interval catchup emission (no stream)
                                            tracing::info!("Starting immediate block catchup for chain {} from block {}", chain, from_block);
                                            active_catchups.insert(chain.clone());
                                            let tm = self.clone();
                                            let chain_for_task = chain.clone();
                                            let catchup_client_for_task = catchup_client.clone();
                                            tokio::spawn(async move {
                                                // Snapshot latest bound
                                                let latest = match catchup_client_for_task
                                                    .provider
                                                    .get_block_number()
                                                    .await
                                                {
                                                    Ok(n) => n,
                                                    Err(e) => {
                                                        tracing::error!("Failed to snapshot latest for catchup on {}: {:?}", chain_for_task, e);
                                                        let _ = tm.command_sender.send(
                                                            TriggerCommand::CatchupCompleted {
                                                                chain: chain_for_task,
                                                            },
                                                        );
                                                        return;
                                                    }
                                                };
                                                let mut height = from_block;
                                                let mut processed: u64 = 0;
                                                while height <= latest {
                                                    tm.recovery_manager
                                                        .record_successful_block(
                                                            &chain_for_task,
                                                            height,
                                                        )
                                                        .await;
                                                    let cmds = tm.process_blocks(
                                                        chain_for_task.clone(),
                                                        height,
                                                    );
                                                    if !cmds.is_empty() {
                                                        let _ = tm.send_dispatcher_commands(cmds);
                                                    }
                                                    height = height.saturating_add(1);
                                                    processed += 1;
                                                    if processed % 500 == 0 {
                                                        tokio::task::yield_now().await;
                                                    }
                                                }
                                                let _ = tm.command_sender.send(
                                                    TriggerCommand::CatchupCompleted {
                                                        chain: chain_for_task,
                                                    },
                                                );
                                            });

                                            // Build filter matching current subscriptions for backfill
                                            let filter = {
                                                use alloy_rpc_types_eth::Filter;
                                                use std::collections::HashSet;
                                                let triggers = self
                                                    .lookup_maps
                                                    .triggers_by_evm_contract_event
                                                    .read()
                                                    .unwrap();
                                                let mut addrs: HashSet<alloy_primitives::Address> =
                                                    HashSet::new();
                                                let mut topic0s: HashSet<alloy_primitives::B256> =
                                                    HashSet::new();
                                                for ((c, addr, event_hash), _ids) in triggers.iter()
                                                {
                                                    if *c == chain {
                                                        addrs.insert(*addr);
                                                        topic0s.insert(
                                                            alloy_primitives::B256::from_slice(
                                                                event_hash.as_slice(),
                                                            ),
                                                        );
                                                    }
                                                }
                                                if let Ok(services) = self.services.list(
                                                    std::ops::Bound::Unbounded,
                                                    std::ops::Bound::Unbounded,
                                                ) {
                                                    for service in services.into_iter() {
                                                        match service.manager {
                                                            ServiceManager::Evm {
                                                                chain: mgr_chain,
                                                                address,
                                                            } => {
                                                                if mgr_chain == chain {
                                                                    addrs.insert(address);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                topic0s.insert(alloy_primitives::B256::from_slice(IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH.as_slice()));
                                                let mut f = Filter::new();
                                                if !addrs.is_empty() {
                                                    f = f.address(
                                                        addrs.into_iter().collect::<Vec<_>>(),
                                                    );
                                                }
                                                if !topic0s.is_empty() {
                                                    f = f.event_signature(
                                                        topic0s.into_iter().collect::<Vec<_>>(),
                                                    );
                                                }
                                                f
                                            };
                                            match catchup::start_event_backfill_stream(
                                                chain.clone(),
                                                catchup_client,
                                                self.recovery_manager.clone(),
                                                filter,
                                                from_block,
                                                self.metrics.clone(),
                                            )
                                            .await
                                            {
                                                Ok(backfill_stream) => {
                                                    multiplexed_stream.push(backfill_stream);
                                                }
                                                Err(err) => {
                                                    tracing::error!("Failed to start event backfill for chain {}: {:?}", chain, err);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to create EVM client for catchup on chain {}: {:?}", chain, e);
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to get endpoint for chain {}: {:?}",
                                            chain,
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        TriggerCommand::CatchupCompleted { chain } => {
                            // Catchup finished: end recovery, clear active flag, and refresh live subscription
                            self.recovery_manager.end_recovery(&chain).await;
                            active_catchups.remove(&chain);
                            let _ = self
                                .command_sender
                                .send(TriggerCommand::ResubscribeEvmChain { chain });
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
                                    let endpoint = chain_config
                                        .query_client_endpoint()
                                        .map_err(|e| TriggerError::EvmClient(chain.clone(), e))?;
                                    let evm_client = EvmQueryClient::new(endpoint)
                                        .await
                                        .map_err(|e| TriggerError::EvmClient(chain.clone(), e))?;

                                    evm_clients.insert(chain.clone(), evm_client.clone());

                                    // Start the EVM event stream
                                    // Build a narrowed filter from registered EVM triggers on this chain
                                    let filter = {
                                        use alloy_rpc_types_eth::Filter;
                                        use std::collections::HashSet;
                                        let triggers = self
                                            .lookup_maps
                                            .triggers_by_evm_contract_event
                                            .read()
                                            .unwrap();
                                        let mut addrs: HashSet<alloy_primitives::Address> =
                                            HashSet::new();
                                        let mut topic0s: HashSet<alloy_primitives::B256> =
                                            HashSet::new();
                                        for ((c, addr, event_hash), _ids) in triggers.iter() {
                                            if *c == chain {
                                                addrs.insert(*addr);
                                                topic0s.insert(alloy_primitives::B256::from_slice(
                                                    event_hash.as_slice(),
                                                ));
                                            }
                                        }
                                        // Include ServiceManager addresses (active services) on this chain
                                        if let Ok(services) = self.services.list(
                                            std::ops::Bound::Unbounded,
                                            std::ops::Bound::Unbounded,
                                        ) {
                                            for service in services.into_iter() {
                                                match service.manager {
                                                    ServiceManager::Evm {
                                                        chain: mgr_chain,
                                                        address,
                                                    } => {
                                                        if mgr_chain == chain {
                                                            addrs.insert(address);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        // Include ServiceURIUpdated topic0
                                        topic0s.insert(alloy_primitives::B256::from_slice(
                                            IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH
                                                .as_slice(),
                                        ));
                                        let mut f = Filter::new();
                                        if !addrs.is_empty() {
                                            f = f.address(addrs.into_iter().collect::<Vec<_>>());
                                        }
                                        if !topic0s.is_empty() {
                                            // Prefer event_signature (topic0) narrowing
                                            f = f.event_signature(
                                                topic0s.into_iter().collect::<Vec<_>>(),
                                            );
                                        }
                                        f
                                    };

                                    match evm_stream::start_evm_stream(
                                        evm_client.clone(),
                                        chain.clone(),
                                        filter,
                                        self.metrics.clone(),
                                        {
                                            let cancel = CancellationToken::new();
                                            evm_event_cancels.insert(chain.clone(), cancel.clone());
                                            cancel
                                        },
                                    )
                                    .await
                                    {
                                        Ok(evm_event_stream) => {
                                            evm_event_streams
                                                .insert(chain.clone(), evm_event_stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to start EVM event stream: {:?}",
                                                err
                                            );
                                            self.recovery_manager.record_stream_error(&chain).await;

                                            // Immediately trigger catchup from last processed block if available
                                            if let Some(state) =
                                                self.recovery_manager.get_state(&chain).await
                                            {
                                                if let Some(last) = state.last_processed_block {
                                                    let _ = self
                                                        .recovery_manager
                                                        .start_recovery(&chain, last + 1)
                                                        .await;
                                                }
                                            }

                                            // Check if we need to trigger catchup
                                            if let Some(recovery_block) =
                                                self.recovery_manager.needs_recovery(&chain).await
                                            {
                                                tracing::warn!("EVM stream failed for chain {}, triggering catchup from block {}", chain, recovery_block);

                                                // Send command to start catchup stream
                                                let _ = self.command_sender.send(
                                                    TriggerCommand::StartCatchup {
                                                        chain: chain.clone(),
                                                        from_block: recovery_block,
                                                    },
                                                );
                                            }

                                            continue;
                                        }
                                    }

                                    // Start the EVM block stream
                                    match evm_stream::start_evm_block_stream(
                                        evm_client.clone(),
                                        chain.clone(),
                                        self.metrics.clone(),
                                    )
                                    .await
                                    {
                                        Ok(evm_block_stream) => {
                                            evm_block_streams
                                                .insert(chain.clone(), evm_block_stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to start EVM block stream: {:?}",
                                                err
                                            );
                                            self.recovery_manager.record_stream_error(&chain).await;

                                            // Immediately trigger catchup from last processed block if available
                                            if let Some(state) =
                                                self.recovery_manager.get_state(&chain).await
                                            {
                                                if let Some(last) = state.last_processed_block {
                                                    let _ = self
                                                        .recovery_manager
                                                        .start_recovery(&chain, last + 1)
                                                        .await;
                                                }
                                            }

                                            // Check if we need to trigger catchup
                                            if let Some(recovery_block) =
                                                self.recovery_manager.needs_recovery(&chain).await
                                            {
                                                tracing::warn!("EVM block stream failed for chain {}, triggering catchup from block {}", chain, recovery_block);

                                                // Send command to start catchup stream
                                                let _ = self.command_sender.send(
                                                    TriggerCommand::StartCatchup {
                                                        chain: chain.clone(),
                                                        from_block: recovery_block,
                                                    },
                                                );
                                            }

                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        TriggerCommand::ResubscribeEvmChain { chain } => {
                            // Only applicable if already listening and the chain is EVM
                            if !listening_chains.contains(&chain) {
                                continue;
                            }
                            if let Some(AnyChainConfig::Evm(_)) =
                                self.chain_configs.read().unwrap().get_chain(&chain)
                            {
                                // Cancel and remove previous keyed event stream
                                if let Some(tok) = evm_event_cancels.remove(&chain) {
                                    tok.cancel();
                                }
                                let _ = evm_event_streams.remove(&chain);

                                // Rebuild filter and resubscribe
                                let filter = {
                                    use alloy_rpc_types_eth::Filter;
                                    use std::collections::HashSet;
                                    let triggers = self
                                        .lookup_maps
                                        .triggers_by_evm_contract_event
                                        .read()
                                        .unwrap();
                                    let mut addrs: HashSet<alloy_primitives::Address> =
                                        HashSet::new();
                                    let mut topic0s: HashSet<alloy_primitives::B256> =
                                        HashSet::new();
                                    for ((c, addr, event_hash), _ids) in triggers.iter() {
                                        if *c == chain {
                                            addrs.insert(*addr);
                                            topic0s.insert(alloy_primitives::B256::from_slice(
                                                event_hash.as_slice(),
                                            ));
                                        }
                                    }
                                    // Include ServiceManager addresses (active services) on this chain
                                    if let Ok(services) = self.services.list(
                                        std::ops::Bound::Unbounded,
                                        std::ops::Bound::Unbounded,
                                    ) {
                                        for service in services.into_iter() {
                                            match service.manager {
                                                ServiceManager::Evm {
                                                    chain: mgr_chain,
                                                    address,
                                                } => {
                                                    if mgr_chain == chain {
                                                        addrs.insert(address);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // Include ServiceURIUpdated topic0
                                    topic0s.insert(alloy_primitives::B256::from_slice(
                                        IWavsServiceManager::ServiceURIUpdated::SIGNATURE_HASH
                                            .as_slice(),
                                    ));
                                    let mut f = Filter::new();
                                    if !addrs.is_empty() {
                                        f = f.address(addrs.into_iter().collect::<Vec<_>>());
                                    }
                                    if !topic0s.is_empty() {
                                        f = f.event_signature(
                                            topic0s.into_iter().collect::<Vec<_>>(),
                                        );
                                    }
                                    f
                                };

                                if let Some(evm_client) = evm_clients.get(&chain).cloned() {
                                    let cancel = CancellationToken::new();
                                    evm_event_cancels.insert(chain.clone(), cancel.clone());
                                    match evm_stream::start_evm_stream(
                                        evm_client,
                                        chain.clone(),
                                        filter,
                                        self.metrics.clone(),
                                        cancel,
                                    )
                                    .await
                                    {
                                        Ok(stream) => {
                                            evm_event_streams.insert(chain.clone(), stream);
                                        }
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to resubscribe EVM stream for {}: {:?}",
                                                chain,
                                                err
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                StreamTriggers::Evm {
                    log,
                    chain: chain_key,
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
                            chain_key.clone(),
                            contract_address,
                            ByteArray::new(**event_hash),
                        )) {
                            let trigger_data = TriggerData::EvmContractEvent {
                                contract_address,
                                chain: chain_key.clone(),
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

                        // Record successful block processing for recovery
                        self.recovery_manager
                            .record_successful_block(&chain_key, block_number)
                            .await;
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
                    // Record successful block processing for recovery
                    self.recovery_manager
                        .record_successful_block(&chain, block_height)
                        .await;

                    // Check if this chain needs recovery (due to missed blocks)
                    if let Some(recovery_block) = self.recovery_manager.needs_recovery(&chain).await
                    {
                        if recovery_block <= block_height {
                            // We've caught up, end recovery mode
                            self.recovery_manager.end_recovery(&chain).await;
                            tracing::info!("Recovery completed for chain {}", chain);
                            // Allow future catchups if needed
                            active_catchups.remove(&chain);
                        }
                    }

                    dispatcher_commands.extend(self.process_blocks(chain, block_height));
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
                        let service = self
                            .services
                            .get(&action.config.service_id)
                            .map_err(TriggerError::Services)?;
                        let event_id = EventId::try_from((&service, action))
                            .map_err(TriggerError::EncodeEventId)?;
                        tracing::debug!(
                            batch = idx + 1,
                            service_id = %action.config.service_id,
                            workflow_id = %action.config.workflow_id,
                            trigger_data = ?action.data,
                            event_id = %event_id,
                            "Trigger action (in this batch)"
                        );
                    }
                }

                self.send_dispatcher_commands(dispatcher_commands)?;
            }
        }

        tracing::debug!("Trigger Manager watcher finished");

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
        let firing_lookup_ids = match self.lookup_maps.block_schedulers.get_mut(&chain) {
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
        config::ChainConfigs, storage::db::RedbStorage, telemetry::TriggerMetrics,
        test_utils::address::rand_address_evm,
    };
    use wavs_types::{
        Component, ComponentDigest, ComponentSource, ServiceManager, SignatureKind, Submit,
        Trigger, TriggerAction, TriggerConfig, TriggerData, Workflow, WorkflowId,
    };

    #[test]
    fn test_add_trigger() {
        let config = Config {
            chains: ChainConfigs::default(),
            ..Default::default()
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let db_storage = RedbStorage::new(temp_dir.path().join("db")).unwrap();
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
