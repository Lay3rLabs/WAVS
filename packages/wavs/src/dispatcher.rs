/***
 *
 * High-level system design
 *
 * The main component is the Dispatcher, which can receive "management" calls via the http server
 * to determine its configuration. It works at the level of "Services" which are independent
 * collections of code and triggers that serve one AVS.
 *
 * Principally the Dispatcher manages workflows by the following system:
 *
 * When the workflow is created, it adds all relevant triggers to the TriggerManager
 * It continually listens to new results from the TriggerManager, and executes them on the WasmEngine.
 * When the WasmEngine has produced the result, it submits it to the destination (typically a ServiceHandler contract).
 *
 * The TriggerManager is meant to be able to handle a large number of
 * async network requests. These may be polling or event-driven (websockets), but there are expected to be quite
 * a few network calls and relatively little computation.
 *
 * The WasmEngine stores a large number of wasm components, indexed by their digest, but all sharing the same WIT entrypoint.
 *
 * Once the results are calculated, they need to be signed and submitted to the chain (typically via the aggregator).
 *
 ***/

use alloy_provider::{DynProvider, ProviderBuilder};
use anyhow::Result;
use futures::{stream, StreamExt};
use iri_string::types::{CreationError, UriString};
use layer_climb::querier::QueryClient;
use std::collections::HashMap;
use std::ops::Bound;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tracing::instrument;
use utils::error::EvmClientError;
use utils::service::fetch_service;
use utils::storage::fs::FileStorage;
use utils::telemetry::{DispatcherMetrics, WavsMetrics};
use wavs_gui_shared::event::TauriEventEmitterExt;
use wavs_types::contracts::cosmwasm::service_manager::ServiceManagerQueryMessages;
use wavs_types::IWavsServiceManager::IWavsServiceManagerInstance;
use wavs_types::{
    AnyChainConfig, ChainConfigError, ChainConfigs, ChainKey, ComponentDigest, ServiceManager,
    Submission, Submit, TriggerData, WorkflowIdError,
};
use wavs_types::{Service, ServiceError, ServiceId, SignerResponse, TriggerAction, WorkflowId};

use crate::config::Config;
use crate::service_registry::{RegistryError, ServiceRegistry};
use crate::services::{Services, ServicesError};
use crate::subsystems::aggregator::error::AggregatorError;
use crate::subsystems::aggregator::{Aggregator, AggregatorCommand};
use crate::subsystems::engine::error::EngineError;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::engine::{
    AggregatorExecuteKind, EngineCommand, EngineManager, EngineResponse,
};
use crate::subsystems::submission::error::SubmissionError;
use crate::subsystems::submission::{SubmissionCommand, SubmissionManager};
use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::{TriggerCommand, TriggerManager};
use crate::{tracing_service_info, AppContext};
use utils::storage::db::{DBError, WavsDb};
use utils::storage::{CAStorage, CAStorageError};

#[derive(Clone)]
pub struct Dispatcher<S: CAStorage> {
    pub trigger_manager: TriggerManager,
    pub engine_manager: EngineManager<S>,
    pub submission_manager: SubmissionManager,
    pub aggregator: Aggregator,
    pub services: Services,
    pub chain_configs: Arc<RwLock<ChainConfigs>>,
    pub metrics: DispatcherMetrics,
    pub ipfs_gateway: String,
    pub subsystem_to_dispatcher_rx: crossbeam::channel::Receiver<DispatcherCommand>,
    pub dispatcher_to_engine_tx: crossbeam::channel::Sender<EngineCommand>,
    pub dispatcher_to_submission_tx: crossbeam::channel::Sender<SubmissionCommand>,
    pub dispatcher_to_aggregator_tx: crossbeam::channel::Sender<AggregatorCommand>,
    pub db_storage: WavsDb,
    pub service_registry: ServiceRegistry,
    /// Cached EVM HTTP providers per chain to avoid creating new connections for each query
    evm_http_providers: Arc<RwLock<HashMap<ChainKey, DynProvider>>>,
    /// Cached Cosmos query clients per chain to avoid creating new connections for each query
    cosmos_query_clients: Arc<RwLock<HashMap<ChainKey, QueryClient>>>,
    pub tauri_handle: TauriHandle,
}

#[derive(Clone)]
pub enum TauriHandle {
    #[cfg(feature = "gui")]
    Real(tauri::AppHandle),
    Mock,
}

impl TauriEventEmitterExt for TauriHandle {
    fn emit_ext<E: wavs_gui_shared::event::TauriEventExt>(
        &self,
        _event: E,
    ) -> Result<(), wavs_gui_shared::error::AppError> {
        match self {
            #[cfg(feature = "gui")]
            TauriHandle::Real(handle) => handle.emit_ext(_event),
            TauriHandle::Mock => Ok(()),
        }
    }
}

#[cfg(feature = "gui")]
impl From<tauri::AppHandle> for TauriHandle {
    fn from(handle: tauri::AppHandle) -> Self {
        TauriHandle::Real(handle)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DispatcherCommand {
    Trigger(TriggerAction),
    ChangeServiceUri {
        service_id: ServiceId,
        uri: UriString,
    },
    EngineResponse(EngineResponse),
    SubmissionResponse(Submission),
    AggregatorExecute {
        submission: Submission,
        service: Service,
        kind: AggregatorExecuteKind,
    },
    SubmissionConfirmed {
        service_id: ServiceId,
        workflow_id: WorkflowId,
        trigger_data: TriggerData,
    },
}

impl Dispatcher<FileStorage> {
    pub fn new(
        config: &Config,
        metrics: WavsMetrics,
        tauri_handle: impl Into<TauriHandle>,
    ) -> Result<Self, DispatcherError> {
        // Create all our channels for communication
        // except dispatcher_to_trigger calls its local stream channel
        let (subsystem_to_dispatcher_tx, subsystem_to_dispatcher_rx) =
            crossbeam::channel::unbounded::<DispatcherCommand>();

        let (dispatcher_to_engine_tx, dispatcher_to_engine_rx) =
            crossbeam::channel::unbounded::<EngineCommand>();

        let (dispatcher_to_submission_tx, dispatcher_to_submission_rx) =
            crossbeam::channel::unbounded::<SubmissionCommand>();

        let (dispatcher_to_aggregator_tx, dispatcher_to_aggregator_rx) =
            crossbeam::channel::unbounded::<AggregatorCommand>();

        let file_storage = FileStorage::new(config.data.join("ca"))?;
        let db_storage = WavsDb::new()?;
        let service_registry = ServiceRegistry::load(&config.data)?;

        let services = Services::new(db_storage.clone());

        let trigger_manager = TriggerManager::new(
            config,
            metrics.trigger,
            services.clone(),
            subsystem_to_dispatcher_tx.clone(),
        )?;

        let app_storage = config.data.join("app");
        let engine = WasmEngine::new(
            file_storage,
            app_storage,
            config.wasm_lru_size,
            config.chains.clone(),
            Some(config.max_wasm_fuel),
            Some(config.max_execution_seconds),
            metrics.engine,
            db_storage.clone(),
            config.ipfs_gateway.clone(),
            config.max_wasm_payload_size,
            config.max_wasm_salt_size,
        );
        let engine_manager = EngineManager::new(
            engine,
            services.clone(),
            dispatcher_to_engine_rx,
            subsystem_to_dispatcher_tx.clone(),
        );

        let submission_manager = SubmissionManager::new(
            config,
            metrics.submission,
            services.clone(),
            dispatcher_to_submission_rx,
            subsystem_to_dispatcher_tx.clone(),
        )?;

        let aggregator = Aggregator::new(
            config,
            metrics.aggregator,
            services.clone(),
            dispatcher_to_aggregator_rx,
            dispatcher_to_aggregator_tx.clone(),
            subsystem_to_dispatcher_tx.clone(),
        )?;

        Ok(Self {
            trigger_manager,
            engine_manager,
            submission_manager,
            aggregator,
            services,
            db_storage,
            service_registry,
            chain_configs: config.chains.clone(),
            metrics: metrics.dispatcher.clone(),
            ipfs_gateway: config.ipfs_gateway.clone(),
            subsystem_to_dispatcher_rx,
            dispatcher_to_engine_tx,
            dispatcher_to_submission_tx,
            dispatcher_to_aggregator_tx,
            evm_http_providers: Arc::new(RwLock::new(HashMap::new())),
            cosmos_query_clients: Arc::new(RwLock::new(HashMap::new())),
            tauri_handle: tauri_handle.into(),
        })
    }
}

impl<S: CAStorage + 'static> Dispatcher<S> {
    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    #[instrument(skip(self, ctx), fields(subsys = "Dispatcher"))]
    pub fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        let mut handles = Vec::new();

        // Start all subsystems
        handles.push(std::thread::spawn({
            let _self = self.clone();
            let ctx = ctx.clone();
            move || {
                _self.trigger_manager.start(ctx);
            }
        }));

        handles.push(std::thread::spawn({
            let _self = self.clone();
            let ctx = ctx.clone();
            move || {
                _self.engine_manager.start(ctx);
            }
        }));

        handles.push(std::thread::spawn({
            let _self = self.clone();
            let ctx = ctx.clone();
            move || {
                _self.submission_manager.start(ctx);
            }
        }));

        handles.push(std::thread::spawn({
            let _self = self.clone();
            let ctx = ctx.clone();
            move || {
                _self.aggregator.start(ctx);
            }
        }));

        // Kill all subsystems on demand
        handles.push(std::thread::spawn({
            let _self = self.clone();
            let ctx = ctx.clone();
            move || {
                ctx.rt.clone().block_on(async move {
                    if ctx.get_kill_receiver().recv().await.is_ok() {
                        tracing::info!("Shutdown signal received, shutting down dispatcher");
                        // shut down trigger manager
                        if let Err(err) = _self
                            .trigger_manager
                            .command_sender
                            .send(TriggerCommand::Kill)
                        {
                            tracing::error!("Error shutting down trigger manager: {:?}", err);
                        }
                        // shut down engine manager
                        if let Err(err) = _self.dispatcher_to_engine_tx.send(EngineCommand::Kill) {
                            tracing::error!("Error sending kill to engine manager: {:?}", err);
                        }
                        // shut down submission manager
                        if let Err(err) = _self
                            .dispatcher_to_submission_tx
                            .send(SubmissionCommand::Kill)
                        {
                            tracing::error!("Error sending kill to submission manager: {:?}", err);
                        }
                        // shut down aggregator
                        if let Err(err) = _self
                            .dispatcher_to_aggregator_tx
                            .send(AggregatorCommand::Kill)
                        {
                            tracing::error!("Error sending kill to aggregator: {:?}", err);
                        }
                    }
                });
            }
        }));

        // handle incoming commands from subsystems
        std::thread::spawn({
            let _self = self.clone();
            let ctx_rt = ctx.rt.clone();
            move || {
                while let Ok(command) = _self.subsystem_to_dispatcher_rx.recv() {
                    match command {
                        DispatcherCommand::Trigger(action) => {
                            let service = match _self.services.get(&action.config.service_id) {
                                Ok(service) => service,
                                Err(err) => {
                                    tracing::error!("{}", err);
                                    continue;
                                }
                            };

                            // Skip paused services early to avoid unnecessary work and
                            // misleading GUI events.
                            if !_self.services.is_active(&action.config.service_id) {
                                tracing::debug!(
                                    service_id = %action.config.service_id,
                                    "Skipping trigger for paused service",
                                );
                                continue;
                            }

                            tracing::debug!(
                                service_id = %action.config.service_id,
                                workflow_id = %action.config.workflow_id,
                                "Dispatcher received trigger action",
                            );

                            if let Err(err) =
                                _self
                                    .tauri_handle
                                    .emit_ext(wavs_gui_shared::event::TriggerEvent {
                                        action: action.clone(),
                                    })
                            {
                                tracing::error!("Error emitting trigger event to GUI: {:?}", err);
                            }
                            if let Err(err) = _self
                                .dispatcher_to_engine_tx
                                .send(EngineCommand::ExecuteOperator { service, action })
                            {
                                tracing::error!("Error sending work to engine: {:?}", err);
                                _self.metrics.channel_closed_errors.add(
                                    1,
                                    &[opentelemetry::KeyValue::new("channel", "engine_work")],
                                );
                            }
                        }
                        DispatcherCommand::ChangeServiceUri { service_id, uri } => {
                            let _self = _self.clone();
                            ctx_rt.spawn(async move {
                                if let Err(err) = _self.change_service(service_id, uri).await {
                                    tracing::error!(
                                        "Error changing service in managers: {:?}",
                                        err
                                    );
                                }
                            });
                        }

                        DispatcherCommand::EngineResponse(response) => match response {
                            EngineResponse::Operator(msg) => {
                                let workflow = match msg.service.workflows.get(msg.workflow_id()) {
                                    Some(wf) => wf,
                                    None => {
                                        tracing::error!(
                                            "Error fetching workflow {} for service {}",
                                            msg.workflow_id(),
                                            msg.service.name
                                        );
                                        continue;
                                    }
                                };

                                match &workflow.submit {
                                    Submit::None => {
                                        tracing::debug!(
                                            "Workflow {} for service {} has no submit action, skipping submission",
                                            msg.workflow_id(),
                                            msg.service.name
                                        );
                                    }
                                    _ => {
                                        if let Err(e) = _self
                                            .dispatcher_to_submission_tx
                                            .send(SubmissionCommand::Submit(msg))
                                        {
                                            tracing::error!(
                                                "Error sending message to submission manager: {:?}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }

                            // This is AFTER aggregator has aggregated, and executed the component
                            EngineResponse::Aggregator {
                                submission,
                                actions,
                                kind,
                            } => {
                                if let Err(e) = _self.dispatcher_to_aggregator_tx.send(
                                    AggregatorCommand::Actions {
                                        submission,
                                        actions,
                                        kind,
                                    },
                                ) {
                                    tracing::error!("Error sending message to aggregator: {:?}", e);
                                }
                            }
                        },

                        DispatcherCommand::SubmissionResponse(submission) => {
                            // This is BEFORE aggregator has even broadcast
                            if let Err(e) = _self
                                .dispatcher_to_aggregator_tx
                                .send(AggregatorCommand::Broadcast(submission))
                            {
                                tracing::error!("Error sending message to aggregator: {:?}", e);
                            }
                        }
                        DispatcherCommand::AggregatorExecute {
                            submission,
                            service,
                            kind,
                        } => {
                            if let Err(err) = _self.dispatcher_to_engine_tx.send(
                                EngineCommand::ExecuteAggregator {
                                    submission,
                                    service,
                                    kind,
                                },
                            ) {
                                tracing::error!("Error sending work to engine: {:?}", err);
                                _self.metrics.channel_closed_errors.add(
                                    1,
                                    &[opentelemetry::KeyValue::new("channel", "engine_work")],
                                );
                            }
                        }
                        DispatcherCommand::SubmissionConfirmed {
                            service_id,
                            workflow_id,
                            trigger_data,
                        } => {
                            if let Err(err) = _self.tauri_handle.emit_ext(
                                wavs_gui_shared::event::SubmissionEvent {
                                    service_id,
                                    workflow_id,
                                    trigger_data,
                                },
                            ) {
                                tracing::error!(
                                    "Error emitting submission event to GUI: {:?}",
                                    err
                                );
                            }
                        }
                    }
                }
            }
        });

        // Restore services from the persisted registry
        let registry_entries = self.service_registry.entries();
        let chain_configs_for_restore = self.chain_configs.read().unwrap().clone();
        let ipfs_gateway_for_restore = self.ipfs_gateway.clone();
        let evm_providers_for_restore = self.evm_http_providers.clone();
        let cosmos_clients_for_restore = self.cosmos_query_clients.clone();

        let initial_services: Vec<Service> = ctx.rt.block_on(async {
            // Fetch all services from chain in parallel (bounded concurrency)
            const MAX_CONCURRENT_RESTORES: usize = 10;
            let fetched: Vec<_> = stream::iter(&registry_entries)
                .map(|entry| {
                    let chain_configs = &chain_configs_for_restore;
                    let ipfs_gateway = &ipfs_gateway_for_restore;
                    let evm_providers = &evm_providers_for_restore;
                    let cosmos_clients = &cosmos_clients_for_restore;
                    async move {
                        let result = query_service_from_address(
                            entry.service_manager.chain().clone(),
                            entry.service_manager.address(),
                            chain_configs,
                            ipfs_gateway,
                            evm_providers,
                            cosmos_clients,
                        )
                        .await;
                        (entry, result)
                    }
                })
                .buffer_unordered(MAX_CONCURRENT_RESTORES)
                .collect::<Vec<_>>()
                .await;

            // Process results sequentially (DB writes and manager registration are not async-safe to parallelize)
            let mut restored = Vec::new();
            for (entry, result) in fetched {
                let service = match result {
                    Ok(s) => s,
                    Err(e) => {
                        // Only remove the registry entry when the on-chain contract is genuinely
                        // absent or has an invalid URI. Transient errors (connection refused,
                        // HTTP 5xx, timeout) are NOT evidence of staleness — keep the entry so
                        // the next startup can retry.
                        let is_genuine_stale = matches!(
                            &e,
                            DispatcherError::AlloyContract(_) | DispatcherError::URICreation(_)
                        );
                        if is_genuine_stale {
                            tracing::warn!(
                                "Failed to restore service for {:?}: {e:#}. Removing stale registry entry.",
                                entry.service_manager
                            );
                            if let Err(rem_err) =
                                self.service_registry.remove(&entry.service_manager)
                            {
                                tracing::error!(
                                    "Failed to remove stale registry entry for {:?}: {rem_err}",
                                    entry.service_manager
                                );
                            }
                        } else {
                            tracing::warn!(
                                "Failed to restore service for {:?}: {e:#}. Keeping registry entry for next startup.",
                                entry.service_manager
                            );
                        }
                        continue;
                    }
                };

                // Check if Path A (cmd_start_wavs) already loaded this service from the
                // settings cache before the HTTP server was up. If so, refresh the stored
                // definition with the authoritative on-chain version but skip manager
                // registration to avoid double-registration errors.
                let already_in_memory = self.services.exists(&service.id()).unwrap_or(false);

                // Always refresh the stored definition with the authoritative on-chain version
                self.services.save(&service)?;

                // Store components
                self.engine_manager
                    .store_components_for_service(&service)
                    .await?;

                if !already_in_memory {
                    // Add to managers with explicit HD index from registry
                    add_service_to_managers(
                        &service,
                        &self.trigger_manager,
                        &self.submission_manager,
                        &self.dispatcher_to_aggregator_tx,
                        Some(entry.hd_index),
                    )?;

                    tracing::info!(
                        "Restored service {} [{:?}] with HD index {}",
                        service.name,
                        service.manager,
                        entry.hd_index
                    );
                } else {
                    tracing::debug!(
                        "Service {} already loaded from settings cache; skipping manager setup",
                        service.name
                    );
                }
                restored.push(service);
            }
            Ok::<_, DispatcherError>(restored)
        })?;

        let total_workflows: usize = initial_services.iter().map(|s| s.workflows.len()).sum();
        tracing::info!(
            "Initializing dispatcher: services={}, workflows={}, components={}",
            initial_services.len(),
            total_workflows,
            self.list_component_digests()?.len()
        );

        // Check ServiceURI for each service at startup and update if needed (bounded concurrency)
        let chain_configs = self.chain_configs.read().unwrap().clone();
        let ipfs_gateway = self.ipfs_gateway.clone();
        let evm_http_providers = self.evm_http_providers.clone();
        let cosmos_query_clients = self.cosmos_query_clients.clone();
        ctx.rt.block_on(async {
            let ipfs_gateway = ipfs_gateway.as_ref();
            let chain_configs = &chain_configs;
            let evm_http_providers = &evm_http_providers;
            let cosmos_query_clients = &cosmos_query_clients;

            // Limit concurrent ServiceURI checks
            const MAX_CONCURRENT_CHECKS: usize = 10;
            let verification_results = stream::iter(&initial_services)
                .map(|service| {
                    let original_service_id = service.id();
                    async move {
                        (
                            original_service_id,
                            check_service_needs_update(
                                service,
                                chain_configs,
                                ipfs_gateway,
                                evm_http_providers,
                                cosmos_query_clients,
                            )
                            .await,
                        )
                    }
                })
                .buffer_unordered(MAX_CONCURRENT_CHECKS)
                .collect::<Vec<_>>()
                .await;

            // Apply updates for services that need them
            for (original_service_id, verification_result) in verification_results {
                match verification_result {
                    Ok(Some(current_service)) => {
                        // Service needs updating - apply the update using change_service_inner
                        if let Err(err) = self
                            .change_service_inner(
                                original_service_id.clone(),
                                current_service.clone(),
                            )
                            .await
                        {
                            tracing::error!(
                                service_id = %original_service_id,
                                error = %err,
                                "Failed to apply service update at startup"
                            );
                        } else {
                            tracing::info!(
                                service_id = %current_service.id(),
                                "ServiceURI updated at startup"
                            );
                        }
                    }
                    Ok(None) => {
                        // No update needed
                    }
                    Err(err) => {
                        tracing::error!(
                            service_id = %original_service_id,
                            error = %err,
                            "Failed to verify ServiceURI at startup, using cached version"
                        );
                    }
                }
            }
        });

        for handle in handles {
            if let Err(err) = handle.join() {
                tracing::error!("Error joining dispatcher thread: {:?}", err);
            }
        }

        Ok(())
    }

    #[instrument(skip(self, source), fields(subsys = "Dispatcher"))]
    pub fn store_component_bytes(
        &self,
        source: Vec<u8>,
    ) -> Result<ComponentDigest, DispatcherError> {
        let digest = self.engine_manager.engine.store_component_bytes(&source)?;
        Ok(digest)
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub fn list_component_digests(&self) -> Result<Vec<ComponentDigest>, DispatcherError> {
        let digests = self.engine_manager.engine.list_digests()?;

        Ok(digests)
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub async fn add_service(
        &self,
        service_manager: ServiceManager,
    ) -> Result<Service, DispatcherError> {
        let chain_configs = self.chain_configs.read().unwrap().clone();
        let service = query_service_from_address(
            service_manager.chain().clone(),
            service_manager.address(),
            &chain_configs,
            &self.ipfs_gateway,
            &self.evm_http_providers,
            &self.cosmos_query_clients,
        )
        .await?;

        self.register_and_add_service(service.clone()).await?;

        // Get current service count for logging
        let current_services = self.services.list(Bound::Unbounded, Bound::Unbounded)?;
        let total_services = current_services.len();
        let total_workflows: usize = current_services.iter().map(|s| s.workflows.len()).sum();

        tracing::info!(service.name = %service.name, service.manager = ?service.manager, workflows = %service.workflows.len(), total_services = %total_services, total_workflows = %total_workflows, "Service registered: {}, workflows={}, total_services={}, total_workflows={}", service.name, service.workflows.len(), total_services, total_workflows);

        Ok(service)
    }

    /// Append service to the persistent registry, then add it to the runtime.
    /// Rolls back the registry entry if adding to the runtime fails.
    pub async fn register_and_add_service(&self, service: Service) -> Result<(), DispatcherError> {
        let service_manager = service.manager.clone();
        let hd_index = self.service_registry.append(service_manager.clone())?;
        if let Err(e) = self.add_service_direct(service, Some(hd_index)).await {
            if let Err(remove_err) = self.service_registry.remove(&service_manager) {
                tracing::error!("Failed to roll back registry entry: {remove_err}");
            }
            return Err(e);
        }
        Ok(())
    }

    // this is public just so we can call it from tests
    #[instrument(skip(self, service), fields(subsys = "Dispatcher", service.name = %service.name, service.manager = ?service.manager))]
    pub async fn add_service_direct(
        &self,
        service: Service,
        hd_index: Option<u32>,
    ) -> Result<(), DispatcherError> {
        let service_id = service.id();
        tracing::info!("Adding service: {}", service.name);
        // Check if service is already registered
        if self.services.exists(&service_id)? {
            return Err(DispatcherError::ServiceRegistered(service_id));
        }

        // Store components
        self.engine_manager
            .store_components_for_service(&service)
            .await?;

        // Store the service
        self.services.save(&service)?;

        // Set up triggers and submissions
        add_service_to_managers(
            &service,
            &self.trigger_manager,
            &self.submission_manager,
            &self.dispatcher_to_aggregator_tx,
            hd_index,
        )?;

        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub fn remove_service(&self, id: ServiceId) -> Result<(), DispatcherError> {
        // Remove from persistent registry first so an IO failure doesn't leave
        // the service already gone from memory but still on disk.
        // Fall back to a registry scan when the service is not in the runtime store —
        // this handles cases where a startup-restore failed (e.g. chain was reset) but
        // the registry entry was not cleaned up, leaving the manager address blocked.
        let sm = self
            .services
            .get(&id)
            .ok()
            .map(|s| s.manager.clone())
            .or_else(|| {
                self.service_registry
                    .entries()
                    .into_iter()
                    .find(|e| ServiceId::from(&e.service_manager) == id)
                    .map(|e| e.service_manager)
            });

        if let Some(sm) = sm {
            self.service_registry.remove(&sm)?;
        }

        self.remove_service_inner(id.clone())?;

        Ok(())
    }

    /// Remove a service from in-memory state without mutating the persistent registry.
    /// Used by `change_service_inner` where the ServiceManager hasn't changed.
    fn remove_service_inner(&self, id: ServiceId) -> Result<(), DispatcherError> {
        self.services.remove(&id)?;
        self.engine_manager.engine.remove_storage(&id);
        self.trigger_manager.remove_service(id.clone())?;
        // no need to remove from submission manager, it has nothing to do

        // Unsubscribe from P2P topic for this service (if P2P is enabled)
        if let Err(err) =
            self.dispatcher_to_aggregator_tx
                .send(AggregatorCommand::UnsubscribeService {
                    service_id: id.clone(),
                })
        {
            tracing::warn!(
                "Failed to send UnsubscribeService command for service {}: {:?}",
                id,
                err
            );
        }

        // Get current service count for logging
        let current_services = self.services.list(Bound::Unbounded, Bound::Unbounded)?;
        let total_workflows: usize = current_services.iter().map(|s| s.workflows.len()).sum();

        tracing_service_info!(
            &self.services,
            id,
            "Removed. Remaining services: {}, remaining workflows: {}",
            current_services.len(),
            total_workflows
        );
        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub fn get_service_signer(
        &self,
        service_id: ServiceId,
    ) -> Result<SignerResponse, DispatcherError> {
        Ok(self.submission_manager.get_service_signer(service_id)?)
    }

    /// Returns a map from ServiceManager → HD index for all persisted registry entries.
    /// Used by `cmd_start_wavs` to restore services with the correct signing key.
    pub fn registry_hd_index_map(&self) -> std::collections::BTreeMap<ServiceManager, u32> {
        self.service_registry
            .entries()
            .into_iter()
            .map(|e| (e.service_manager, e.hd_index))
            .collect()
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    async fn change_service(
        &self,
        service_id: ServiceId,
        uri: UriString,
    ) -> Result<(), DispatcherError> {
        let service = fetch_service(&uri, &self.ipfs_gateway)
            .await
            .map_err(DispatcherError::FetchService)?;

        self.change_service_inner(service_id, service).await
    }

    #[instrument(skip(self, service), fields(subsys = "Dispatcher"))]
    async fn change_service_inner(
        &self,
        service_id: ServiceId,
        service: Service,
    ) -> Result<(), DispatcherError> {
        if service.id() != service_id {
            return Err(DispatcherError::ChangeIdMismatch {
                old_id: service_id,
                new_id: service.id(),
            });
        }

        let SignerResponse::Secp256k1 { hd_index, .. } = self
            .submission_manager
            .get_service_signer(service_id.clone())?;

        if tracing::enabled!(tracing::Level::INFO) {
            let old_service = self.services.get(&service_id)?;

            tracing::info!("Changing service from {:?} to {:?}", old_service, service);
            tracing::info!("hash {} to {}", old_service.hash()?, service.hash()?);
        }

        // We can't exactly just remove the service and then call `add_service_direct` here because it's async
        // and the runtime may delay calling it, thereby introducing a window where the service is gone.
        // so we do the same steps manually and call the async part of the flow (adding components)
        // _before_ removing the service.

        // Store components
        self.engine_manager
            .store_components_for_service(&service)
            .await?;

        // Remove the old service from in-memory state only (ServiceManager hasn't changed,
        // so no registry mutation needed)
        self.remove_service_inner(service_id.clone())?;

        // Store the service BEFORE setting up triggers/P2P subscription
        // This ensures the service is in the database before any triggers can fire
        // or P2P catch-up can deliver submissions for this service
        self.services.save(&service)?;

        // Set up triggers and submissions
        add_service_to_managers(
            &service,
            &self.trigger_manager,
            &self.submission_manager,
            &self.dispatcher_to_aggregator_tx,
            Some(hd_index),
        )?;

        Ok(())
    }
}

/// Standalone function to verify service URI
/// Returns Some(Service) with the new Service if the service needs updating, None if it's up to date
async fn check_service_needs_update(
    service: &Service,
    chain_configs: &ChainConfigs,
    ipfs_gateway: &str,
    evm_http_providers: &Arc<RwLock<HashMap<ChainKey, DynProvider>>>,
    cosmos_query_clients: &Arc<RwLock<HashMap<ChainKey, QueryClient>>>,
) -> Result<Option<Service>, DispatcherError> {
    let service_id = service.id();
    let cached_hash = service.hash()?;

    // Get current service from contract
    let current_service = query_service_from_address(
        service.manager.chain().clone(),
        service.manager.address(),
        chain_configs,
        ipfs_gateway,
        evm_http_providers,
        cosmos_query_clients,
    )
    .await?;

    let current_hash = current_service.hash()?;

    if current_hash != cached_hash {
        tracing::info!(
            service_id = %service_id,
            cached_hash = %cached_hash,
            current_hash = %current_hash,
            "Service definition differs from contract, updating"
        );

        Ok(Some(current_service))
    } else {
        Ok(None) // No update needed
    }
}

async fn query_service_from_address(
    chain: ChainKey,
    address: layer_climb::prelude::Address,
    chain_configs: &ChainConfigs,
    ipfs_gateway: &str,
    evm_http_providers: &Arc<RwLock<HashMap<ChainKey, DynProvider>>>,
    cosmos_query_clients: &Arc<RwLock<HashMap<ChainKey, QueryClient>>>,
) -> Result<Service, DispatcherError> {
    // Get the chain config
    let chain_config = chain_configs.get_chain(&chain).ok_or_else(|| {
        DispatcherError::Config(format!("Could not get chain config for chain {chain}"))
    })?;

    // Handle different chain types
    let service_uri = match chain_config {
        AnyChainConfig::Evm(evm_config) => {
            // Get or create cached provider for this chain
            let provider = {
                let providers = evm_http_providers.read().unwrap();
                providers.get(&chain).cloned()
            };

            let provider = match provider {
                Some(p) => p,
                None => {
                    // Get the HTTP endpoint, required for contract calls
                    let http_endpoint = evm_config.http_endpoint.clone().ok_or_else(|| {
                        DispatcherError::Config(format!(
                            "No HTTP endpoint configured for chain {chain}"
                        ))
                    })?;

                    // Create a provider using the HTTP endpoint
                    let new_provider = DynProvider::new(ProviderBuilder::new().connect_http(
                        reqwest::Url::parse(&http_endpoint).unwrap_or_else(|_| {
                            panic!("Could not parse http endpoint {}", http_endpoint)
                        }),
                    ));

                    // Cache the provider
                    let mut providers = evm_http_providers.write().unwrap();
                    providers.insert(chain.clone(), new_provider.clone());
                    new_provider
                }
            };

            let contract = IWavsServiceManagerInstance::new(
                address
                    .try_into()
                    .map_err(DispatcherError::AddressConversion)?,
                provider,
            );

            let service_uri = contract.getServiceURI().call().await?;
            service_uri
        }
        AnyChainConfig::Cosmos(config) => {
            // Get or create cached query client for this chain
            let query_client = {
                let clients = cosmos_query_clients.read().unwrap();
                clients.get(&chain).cloned()
            };

            let query_client = match query_client {
                Some(c) => c,
                None => {
                    // Create a new query client
                    let new_client = QueryClient::new(config.into(), None)
                        .await
                        .map_err(DispatcherError::CosmosQuery)?;

                    // Cache the client
                    let mut clients = cosmos_query_clients.write().unwrap();
                    clients.insert(chain.clone(), new_client.clone());
                    new_client
                }
            };

            let service_uri: String = query_client
                .contract_smart(&address, &ServiceManagerQueryMessages::WavsServiceUri {})
                .await
                .map_err(DispatcherError::CosmosQuery)?;

            service_uri
        }
        AnyChainConfig::Solana(_) => {
            // slice 2 / v2: Solana submission path. v1 is trigger-only —
            // services with a Solana service manager are not yet supported.
            return Err(DispatcherError::Config(format!(
                "Solana service managers are not yet supported (chain {chain}, slice 2/v2)"
            )));
        }
    };

    let service_uri = UriString::try_from(service_uri)?;

    // Fetch the service JSON from the URI
    let service = fetch_service(&service_uri, ipfs_gateway)
        .await
        .map_err(DispatcherError::FetchService)?;

    Ok(service)
}

// called at init and when a new service is added
fn add_service_to_managers(
    service: &Service,
    triggers: &TriggerManager,
    submissions: &SubmissionManager,
    // needs to be through channel because subscription is async
    aggregator_tx: &crossbeam::channel::Sender<AggregatorCommand>,
    hd_index: Option<u32>,
) -> Result<(), DispatcherError> {
    if let Err(err) = submissions.add_service_key(service.id(), hd_index) {
        tracing::error!("Error adding service to submission manager: {:?}", err);
        return Err(err.into());
    }

    if let Err(err) = triggers.add_service(service) {
        tracing::error!("Error adding service to trigger manager: {:?}", err);
        return Err(err.into());
    }

    // Subscribe to P2P topic for this service (if P2P is enabled)
    if let Err(err) = aggregator_tx.send(AggregatorCommand::SubscribeService {
        service_id: service.id(),
    }) {
        tracing::warn!(
            "Failed to send SubscribeService command for service {}: {:?}",
            service.name,
            err
        );
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum DispatcherError {
    #[error("Service {0} already registered")]
    ServiceRegistered(ServiceId),

    #[error("Evm: {0}")]
    EvmClient(#[from] EvmClientError),

    #[error("URI creation error: {0}")]
    URICreation(#[from] CreationError<String>),

    #[error("{0:?}")]
    UnknownService(#[from] ServicesError),

    #[error("Invalid WorkflowId: {0}")]
    ID(#[from] WorkflowIdError),

    #[error("DB: {0}")]
    DB(#[from] DBError),

    #[error("DB Storage: {0}")]
    DBStorage(#[source] anyhow::Error),

    #[error("DB: {0}")]
    CA(#[from] CAStorageError),

    #[error("Engine: {0}")]
    Engine(#[from] EngineError),

    #[error("Trigger: {0}")]
    Trigger(#[from] TriggerError),

    #[error("Submission: {0}")]
    Submission(#[from] SubmissionError),

    #[error("Aggregator: {0}")]
    Aggregator(#[from] AggregatorError),

    #[error("Chain config error: {0}")]
    ChainConfig(#[from] ChainConfigError),

    #[error("Alloy contract error: {0}")]
    AlloyContract(#[from] alloy_contract::Error),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("No registry domain provided in configuration")]
    NoRegistry,

    #[error("Unknown component digest: {0}")]
    UnknownComponentDigest(ComponentDigest),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Service change: id mismatch, from {old_id} to {new_id}")]
    ChangeIdMismatch {
        old_id: ServiceId,
        new_id: ServiceId,
    },

    #[error("could not encode EventId {0:?}")]
    EncodeEventId(anyhow::Error),

    #[error("Failed to fetch service: {0}")]
    FetchService(anyhow::Error),

    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    #[error("Address conversion error: {0}")]
    AddressConversion(anyhow::Error),

    #[error("Cosmos query error: {0}")]
    CosmosQuery(anyhow::Error),

    #[error("Service registry: {0}")]
    Registry(#[from] RegistryError),
}
