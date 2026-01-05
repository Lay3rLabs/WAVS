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

use alloy_provider::ProviderBuilder;
use anyhow::Result;
use futures::{stream, StreamExt};
use iri_string::types::{CreationError, UriString};
use layer_climb::querier::QueryClient;
use std::ops::Bound;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tracing::instrument;
use utils::error::EvmClientError;
use utils::service::fetch_service;
use utils::storage::fs::FileStorage;
use utils::telemetry::{DispatcherMetrics, WavsMetrics};
use wavs_types::contracts::cosmwasm::service_manager::ServiceManagerQueryMessages;
use wavs_types::IWavsServiceManager::IWavsServiceManagerInstance;
use wavs_types::{
    AnyChainConfig, ChainConfigError, ChainConfigs, ChainKey, ComponentDigest, ServiceManager,
    WorkflowIdError,
};
use wavs_types::{Service, ServiceError, ServiceId, SignerResponse, TriggerAction};

use crate::config::Config;
use crate::services::{Services, ServicesError};
use crate::subsystems::engine::error::EngineError;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::engine::{EngineCommand, EngineManager};
use crate::subsystems::submission::chain_message::ChainMessage;
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
    pub services: Services,
    pub chain_configs: Arc<RwLock<ChainConfigs>>,
    pub metrics: DispatcherMetrics,
    pub ipfs_gateway: String,
    pub trigger_to_dispatcher_rx: crossbeam::channel::Receiver<DispatcherCommand>,
    pub dispatcher_to_engine_tx: crossbeam::channel::Sender<EngineCommand>,
    pub engine_to_dispatcher_rx: crossbeam::channel::Receiver<ChainMessage>,
    pub dispatcher_to_submission_tx: crossbeam::channel::Sender<SubmissionCommand>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum DispatcherCommand {
    Trigger(TriggerAction),
    ChangeServiceUri {
        service_id: ServiceId,
        uri: UriString,
    },
}

impl Dispatcher<FileStorage> {
    pub fn new(config: &Config, metrics: WavsMetrics) -> Result<Self, DispatcherError> {
        // Create all our channels for communication
        // except dispatcher_to_trigger calls its local stream channel
        let (trigger_to_dispatcher_tx, trigger_to_dispatcher_rx) =
            crossbeam::channel::unbounded::<DispatcherCommand>();

        let (dispatcher_to_engine_tx, dispatcher_to_engine_rx) =
            crossbeam::channel::unbounded::<EngineCommand>();
        let (engine_to_dispatcher_tx, engine_to_dispatcher_rx) =
            crossbeam::channel::unbounded::<ChainMessage>();

        let (dispatcher_to_submission_tx, dispatcher_to_submission_rx) =
            crossbeam::channel::unbounded::<SubmissionCommand>();

        let file_storage = FileStorage::new(config.data.join("ca"))?;
        let db_storage = WavsDb::new()?;

        let services = Services::new(db_storage.clone());

        let trigger_manager = TriggerManager::new(
            config,
            metrics.trigger,
            services.clone(),
            trigger_to_dispatcher_tx,
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
            db_storage,
            config.ipfs_gateway.clone(),
        );
        let engine_manager = EngineManager::new(
            engine,
            services.clone(),
            dispatcher_to_engine_rx,
            engine_to_dispatcher_tx,
        );

        let submission_manager = SubmissionManager::new(
            config,
            metrics.submission,
            services.clone(),
            dispatcher_to_submission_rx,
        )?;

        Ok(Self {
            trigger_manager,
            engine_manager,
            submission_manager,
            services,
            chain_configs: config.chains.clone(),
            metrics: metrics.dispatcher.clone(),
            ipfs_gateway: config.ipfs_gateway.clone(),
            trigger_to_dispatcher_rx,
            dispatcher_to_engine_tx,
            engine_to_dispatcher_rx,
            dispatcher_to_submission_tx,
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
                    }
                });
            }
        }));

        // handle incoming commands from trigger manager
        std::thread::spawn({
            let _self = self.clone();
            let ctx_rt = ctx.rt.clone();
            move || {
                while let Ok(command) = _self.trigger_to_dispatcher_rx.recv() {
                    match command {
                        DispatcherCommand::Trigger(action) => {
                            let service = match _self.services.get(&action.config.service_id) {
                                Ok(service) => service,
                                Err(err) => {
                                    tracing::error!("{}", err);
                                    continue;
                                }
                            };

                            tracing::info!(
                                service_id = %action.config.service_id,
                                workflow_id = %action.config.workflow_id,
                                "Dispatcher received trigger action",
                            );

                            #[cfg(feature = "rerun")]
                            wavs_rerun::log_packet_flow(
                                wavs_rerun::NODE_DISPATCHER,
                                wavs_rerun::NODE_ENGINE,
                                &action.config.workflow_id.to_string(),
                                &action.config.service_id.to_string(),
                                None,
                            );

                            if let Err(err) = _self
                                .dispatcher_to_engine_tx
                                .send(EngineCommand::Execute { service, action })
                            {
                                tracing::error!("Error sending work to engine: {:?}", err);
                                // blocking_send only fails if the receiver has been dropped (channel closed)
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
                    }
                }
            }
        });

        // handle incoming commands from engine manager
        std::thread::spawn({
            let _self = self.clone();
            move || {
                while let Ok(msg) = _self.engine_to_dispatcher_rx.recv() {
                    #[cfg(feature = "rerun")]
                    wavs_rerun::log_packet_flow(
                        wavs_rerun::NODE_DISPATCHER,
                        wavs_rerun::NODE_SUBMISSION,
                        &msg.envelope.eventId.to_string(),
                        &msg.workflow_id.to_string(),
                        None,
                    );

                    if let Err(e) = _self
                        .dispatcher_to_submission_tx
                        .send(SubmissionCommand::Submit(msg))
                    {
                        tracing::error!("Error sending message to submission manager: {:?}", e);
                    }
                }
            }
        });

        // populate the initial triggers
        let initial_services = self.services.list(Bound::Unbounded, Bound::Unbounded)?;
        let total_workflows: usize = initial_services.iter().map(|s| s.workflows.len()).sum();
        tracing::info!(
            "Initializing dispatcher: services={}, workflows={}, components={}",
            initial_services.len(),
            total_workflows,
            self.list_component_digests()?.len()
        );

        for service in initial_services.iter() {
            add_service_to_managers(
                service,
                &self.trigger_manager,
                &self.submission_manager,
                None,
            )?;
        }

        // Check ServiceURI for each service at startup and update if needed (bounded concurrency)
        let chain_configs = self.chain_configs.read().unwrap().clone();
        let ipfs_gateway = self.ipfs_gateway.clone();
        ctx.rt.block_on(async {
            let ipfs_gateway = ipfs_gateway.as_ref();
            let chain_configs = &chain_configs;

            // Limit concurrent ServiceURI checks
            const MAX_CONCURRENT_CHECKS: usize = 10;
            let verification_results = stream::iter(&initial_services)
                .map(|service| {
                    let original_service_id = service.id();
                    async move {
                        (
                            original_service_id,
                            check_service_needs_update(service, chain_configs, ipfs_gateway).await,
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
        let (chain, address) = match service_manager {
            ServiceManager::Evm { chain, address } => {
                (chain, layer_climb::prelude::Address::from(address))
            }
            ServiceManager::Cosmos { chain, address } => {
                (chain, layer_climb::prelude::Address::from(address))
            }
        };
        let chain_configs = self.chain_configs.read().unwrap().clone();
        let service =
            query_service_from_address(chain, address, &chain_configs, &self.ipfs_gateway).await?;

        self.add_service_direct(service.clone()).await?;

        // Get current service count for logging
        let current_services = self.services.list(Bound::Unbounded, Bound::Unbounded)?;
        let total_services = current_services.len();
        let total_workflows: usize = current_services.iter().map(|s| s.workflows.len()).sum();

        tracing::info!(service.name = %service.name, service.manager = ?service.manager, workflows = %service.workflows.len(), total_services = %total_services, total_workflows = %total_workflows, "Service registered: {} [{:?}], workflows={}, total_services={}, total_workflows={}", service.name, service.manager, service.workflows.len(), total_services, total_workflows);

        Ok(service)
    }

    // this is public just so we can call it from tests
    #[instrument(skip(self), fields(subsys = "Dispatcher", service.name = %service.name, service.manager = ?service.manager))]
    pub async fn add_service_direct(&self, service: Service) -> Result<(), DispatcherError> {
        let service_id = service.id();
        tracing::info!("Adding service: {} [{:?}]", service.name, service.manager);
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
            None,
        )?;

        Ok(())
    }

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
    pub fn remove_service(&self, id: ServiceId) -> Result<(), DispatcherError> {
        self.services.remove(&id)?;
        self.engine_manager.engine.remove_storage(&id);
        self.trigger_manager.remove_service(id.clone())?;
        // no need to remove from submission manager, it has nothing to do

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

    #[instrument(skip(self), fields(subsys = "Dispatcher"))]
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

        // Remove the old service - after this, no await points until the new service is added
        self.remove_service(service_id.clone())?;

        // Set up triggers and submissions
        add_service_to_managers(
            &service,
            &self.trigger_manager,
            &self.submission_manager,
            Some(hd_index),
        )?;

        // Store the service
        self.services.save(&service)?;

        Ok(())
    }
}

/// Standalone function to verify service URI
/// Returns Some(Service) with the new Service if the service needs updating, None if it's up to date
async fn check_service_needs_update(
    service: &Service,
    chain_configs: &ChainConfigs,
    ipfs_gateway: &str,
) -> Result<Option<Service>, DispatcherError> {
    let service_id = service.id();
    let cached_hash = service.hash()?;

    // Get current service from contract
    let current_service = match &service.manager {
        ServiceManager::Evm { chain, address } => {
            query_service_from_address(
                chain.clone(),
                (*address).into(),
                chain_configs,
                ipfs_gateway,
            )
            .await?
        }
        ServiceManager::Cosmos { chain, address } => {
            query_service_from_address(
                chain.clone(),
                address.clone().into(),
                chain_configs,
                ipfs_gateway,
            )
            .await?
        }
    };

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
) -> Result<Service, DispatcherError> {
    // Get the chain config
    let chain_config = chain_configs.get_chain(&chain).ok_or_else(|| {
        DispatcherError::Config(format!("Could not get chain config for chain {chain}"))
    })?;

    // Handle different chain types
    let service_uri = match chain_config {
        AnyChainConfig::Evm(evm_config) => {
            // Get the HTTP endpoint, required for contract calls
            let http_endpoint = evm_config.http_endpoint.clone().ok_or_else(|| {
                DispatcherError::Config(format!("No HTTP endpoint configured for chain {chain}"))
            })?;

            // Create a provider using the HTTP endpoint
            let provider = ProviderBuilder::new().connect_http(
                reqwest::Url::parse(&http_endpoint)
                    .unwrap_or_else(|_| panic!("Could not parse http endpoint {}", http_endpoint)),
            );

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
            let query_client = QueryClient::new(config.into(), None)
                .await
                .map_err(DispatcherError::CosmosQuery)?;

            let service_uri: String = query_client
                .contract_smart(&address, &ServiceManagerQueryMessages::WavsServiceUri {})
                .await
                .map_err(DispatcherError::CosmosQuery)?;

            service_uri
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
}
