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
 * The TriggerManager has it's own internal runtime and is meant to be able to handle a large number of
 * async network requests. These may be polling or event-driven (websockets), but there are expected to be quite
 * a few network calls and relatively little computation.
 *
 * The WasmEngine stores a large number of wasm components, indexed by their digest.
 * It should be able to quickly execute any of them, via a number of predefined wit component interfaces.
 * We do want to limit the number of wasmtime instances at once, and so we use a capped rayon threadpool.
 *
 * Once the results are calculated, they need to be signed and submitted to the chain (typically via the aggregator).
 *
 ***/

use alloy_provider::ProviderBuilder;
use anyhow::Result;
use layer_climb::prelude::Address;
use redb::ReadableTable;
use std::ops::Bound;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::config::{AnyChainConfig, ChainConfigs};
use utils::error::ChainConfigError;
use utils::service::fetch_service;
use utils::storage::fs::FileStorage;
use utils::telemetry::{DispatcherMetrics, WavsMetrics};
use wavs_types::IWavsServiceManager::IWavsServiceManagerInstance;
use wavs_types::{
    ChainName, Digest, IDError, Service, ServiceID, SigningKeyResponse, TriggerAction,
    TriggerConfig,
};

use crate::config::Config;
use crate::subsystems::engine::error::EngineError;
use crate::subsystems::engine::wasm_engine::WasmEngine;
use crate::subsystems::engine::EngineManager;
use crate::subsystems::submission::chain_message::ChainMessage;
use crate::subsystems::submission::error::SubmissionError;
use crate::subsystems::submission::SubmissionManager;
use crate::subsystems::trigger::error::TriggerError;
use crate::subsystems::trigger::TriggerManager;
use crate::AppContext;
use utils::storage::db::{DBError, RedbStorage, Table, JSON};
use utils::storage::{CAStorage, CAStorageError};
use wasm_pkg_common::Error as RegistryError;

pub const TRIGGER_CHANNEL_SIZE: usize = 100;
pub const ENGINE_CHANNEL_SIZE: usize = 20;
pub const SUBMISSION_CHANNEL_SIZE: usize = 20;

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

pub struct Dispatcher<S: CAStorage> {
    pub trigger_manager: TriggerManager,
    pub engine_manager: EngineManager<S>,
    pub submission_manager: SubmissionManager,
    pub db_storage: Arc<RedbStorage>,
    pub chain_configs: ChainConfigs,
    pub metrics: DispatcherMetrics,
    pub ipfs_gateway: String,
}

impl Dispatcher<FileStorage> {
    pub fn new(config: &Config, metrics: WavsMetrics) -> Result<Self, DispatcherError> {
        let file_storage = FileStorage::new(config.data.join("ca"))?;
        let db_storage = Arc::new(RedbStorage::new(config.data.join("db"))?);

        let trigger_manager = TriggerManager::new(config, metrics.trigger)?;

        let app_storage = config.data.join("app");
        let engine = WasmEngine::new(
            file_storage,
            app_storage,
            config.wasm_lru_size,
            config.chains.clone(),
            Some(config.max_wasm_fuel),
            Some(config.max_execution_seconds),
            metrics.engine,
        );
        let engine_manager = EngineManager::new(engine, config.wasm_threads);

        let submission_manager = SubmissionManager::new(config, metrics.submission)?;

        Ok(Self {
            trigger_manager,
            engine_manager,
            submission_manager,
            db_storage,
            chain_configs: config.chains.clone(),
            metrics: metrics.dispatcher.clone(),
            ipfs_gateway: config.ipfs_gateway.clone(),
        })
    }
}

impl<S: CAStorage + 'static> Dispatcher<S> {
    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "Dispatcher"))]
    pub fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        // Trigger is pipeline start
        let mut actions_in = self.trigger_manager.start(ctx.clone())?;
        // Next is the local (blocking) processing
        let (work_sender, work_receiver) =
            mpsc::channel::<(TriggerAction, Service)>(ENGINE_CHANNEL_SIZE);
        let (wasi_result_sender, wasi_result_receiver) =
            mpsc::channel::<ChainMessage>(SUBMISSION_CHANNEL_SIZE);
        // Then the engine processing
        self.engine_manager
            .start(ctx.clone(), work_receiver, wasi_result_sender);
        // And pipeline finishes with submission
        self.submission_manager
            .start(ctx.clone(), wasi_result_receiver)?;

        // populate the initial triggers
        let initial_services = self.list_services(Bound::Unbounded, Bound::Unbounded)?;
        let total_workflows: usize = initial_services.iter().map(|s| s.workflows.len()).sum();
        tracing::info!(
            "Initializing dispatcher: services={}, workflows={}, components={}",
            initial_services.len(),
            total_workflows,
            self.list_component_digests()?.len()
        );
        for service in initial_services {
            ctx.rt.block_on(async {
                add_service_to_managers(service, &self.trigger_manager, &self.submission_manager)
                    .await
            })?;
        }

        // since triggers listens to the async kill signal handler and closes the channel when
        // it is triggered, we don't need to jump through hoops here to make an async block to listen.
        // Just waiting for the channel to close is enough.

        // This reads the actions, extends them with the local service data, and passes
        // the combined info down to the EngineRunner to work.
        while let Some(action) = actions_in.blocking_recv() {
            tracing::info!(
                "Dispatcher received trigger action: service_id={}, workflow_id={}",
                action.config.service_id,
                action.config.workflow_id
            );

            let service = match self
                .db_storage
                .get(SERVICE_TABLE, action.config.service_id.as_ref())?
            {
                Some(service) => service.value(),
                None => {
                    let err = DispatcherError::UnknownService(action.config.service_id.clone());
                    tracing::error!("{}", err);
                    continue;
                }
            };
            if let Err(err) = work_sender.blocking_send((action, service)) {
                tracing::error!("Error sending work to engine: {:?}", err);
            }
        }

        // Note: closing channel doesn't let receiver read all buffered messages, but immediately shuts it down
        // https://docs.rs/tokio/latest/tokio/sync/mpsc/fn.channel.html
        // Similarly, if Sender is disconnected while trying to recv,
        // the recv method will return None.

        // see https://stackoverflow.com/questions/65501193/is-it-possible-to-preserve-items-in-a-tokio-mpsc-when-the-last-sender-is-dropped
        // and it seems like they should be delivered...
        // https://github.com/tokio-rs/tokio/issues/6053

        // FIXME: this sleep is a hack to make sure the messages are delivered
        // is there a better way to do this?
        // (in production, this is only hit in shutdown, so not so important, but it causes annoying test failures)
        tracing::debug!("no more work in dispatcher, channel closing");
        std::thread::sleep(Duration::from_millis(500));

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    pub fn store_component_bytes(&self, source: Vec<u8>) -> Result<Digest, DispatcherError> {
        let digest = self.engine_manager.engine.store_component_bytes(&source)?;
        Ok(digest)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    pub fn list_component_digests(&self) -> Result<Vec<Digest>, DispatcherError> {
        let digests = self.engine_manager.engine.list_digests()?;

        Ok(digests)
    }

    pub async fn add_service(
        &self,
        chain_name: ChainName,
        address: Address,
    ) -> Result<(), DispatcherError> {
        let service = query_service_from_address(
            chain_name,
            address,
            &self.chain_configs,
            &self.ipfs_gateway,
        )
        .await?;

        // persist it in storage if not there yet
        if self
            .db_storage
            .get(SERVICE_TABLE, service.id.as_ref())?
            .is_some()
        {
            return Err(DispatcherError::ServiceRegistered(service.id));
        }

        for workflow in service.workflows.values() {
            self.engine_manager
                .engine
                .store_component_from_source(&workflow.component.source)
                .await?;
        }

        self.db_storage
            .set(SERVICE_TABLE, service.id.as_ref(), &service)?;

        add_service_to_managers(
            service.clone(),
            &self.trigger_manager,
            &self.submission_manager,
        )
        .await?;

        // Get current service count for logging
        let current_services = self.list_services(Bound::Unbounded, Bound::Unbounded)?;
        let total_services = current_services.len();
        let total_workflows: usize = current_services.iter().map(|s| s.workflows.len()).sum();

        tracing::info!("Service registered: service_id={}, workflows={}, total_services={}, total_workflows={}", 
            service.id, service.workflows.len(), total_services, total_workflows);

        Ok(())
    }

    pub async fn add_service_direct(&self, service: Service) -> Result<(), DispatcherError> {
        // Check if service is already registered
        if self
            .db_storage
            .get(SERVICE_TABLE, service.id.as_ref())?
            .is_some()
        {
            return Err(DispatcherError::ServiceRegistered(service.id));
        }

        // Store components
        for workflow in service.workflows.values() {
            self.engine_manager
                .engine
                .store_component_from_source(&workflow.component.source)
                .await?;
        }

        // Store the service
        self.db_storage
            .set(SERVICE_TABLE, service.id.as_ref(), &service)?;

        // Set up triggers and submissions
        add_service_to_managers(service, &self.trigger_manager, &self.submission_manager).await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    pub fn remove_service(&self, id: ServiceID) -> Result<(), DispatcherError> {
        self.db_storage.remove(SERVICE_TABLE, id.as_ref())?;
        self.engine_manager.engine.remove_storage(&id);
        self.trigger_manager.remove_service(id.clone())?;
        // no need to remove from submission manager, it has nothing to do

        // Get current service count for logging
        let current_services = self.list_services(Bound::Unbounded, Bound::Unbounded)?;
        let total_workflows: usize = current_services.iter().map(|s| s.workflows.len()).sum();

        tracing::info!(
            "Service removed: service_id={}, remaining_services={}, remaining_workflows={}",
            id,
            current_services.len(),
            total_workflows
        );

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    pub fn list_services(
        &self,
        bounds_start: Bound<&str>,
        bounds_end: Bound<&str>,
    ) -> Result<Vec<Service>, DispatcherError> {
        let res = self
            .db_storage
            .map_table_read(SERVICE_TABLE, |table| match table {
                // TODO: try to refactor. There's a couple areas of improvement:
                //
                // 1. just taking in a RangeBounds<&str> instead of two Bound<&str>
                // 2. just calling `.range()` on the range once
                Some(table) => match (bounds_start, bounds_end) {
                    (Bound::Unbounded, Bound::Unbounded) => {
                        let res = table
                            .iter()?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                    (Bound::Unbounded, Bound::Included(y)) => {
                        let res = table
                            .range(..=y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Unbounded, Bound::Excluded(y)) => {
                        let res = table
                            .range(..y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Unbounded) => {
                        let res = table
                            .range(x..)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Unbounded) => {
                        let res = table
                            .range(x..)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Included(y)) => {
                        let res = table
                            .range(x..=y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Included(x), Bound::Excluded(y)) => {
                        let res = table
                            .range(x..y)?
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;

                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Included(y)) => {
                        let res = table
                            .range(x..=y)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                    (Bound::Excluded(x), Bound::Excluded(y)) => {
                        let res = table
                            .range(x..y)?
                            .skip(1)
                            .map(|i| i.map(|(_, value)| value.value()))
                            .collect::<Result<Vec<_>, redb::StorageError>>()?;
                        Ok(res)
                    }
                },
                None => Ok(Vec::new()),
            })?;

        Ok(res)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    pub fn get_service_key(
        &self,
        service_id: ServiceID,
    ) -> Result<SigningKeyResponse, DispatcherError> {
        Ok(self.submission_manager.get_service_key(service_id)?)
    }
}

async fn query_service_from_address(
    chain_name: ChainName,
    address: Address,
    chain_configs: &ChainConfigs,
    ipfs_gateway: &str,
) -> Result<Service, DispatcherError> {
    // Get the chain config
    let chain = chain_configs.get_chain(&chain_name)?.ok_or_else(|| {
        DispatcherError::Config(format!(
            "Could not get chain config for chain {}",
            chain_name
        ))
    })?;

    // Handle different chain types
    match chain {
        AnyChainConfig::Evm(evm_config) => {
            // Get the HTTP endpoint, required for contract calls
            let http_endpoint = evm_config.http_endpoint.clone().ok_or_else(|| {
                DispatcherError::Config(format!(
                    "No HTTP endpoint configured for chain {}",
                    chain_name
                ))
            })?;

            // Create a provider using the HTTP endpoint
            let provider = ProviderBuilder::new().connect_http(
                reqwest::Url::parse(&http_endpoint)
                    .unwrap_or_else(|_| panic!("Could not parse http endpoint {}", http_endpoint)),
            );

            let contract = IWavsServiceManagerInstance::new(address.try_into()?, provider);

            let service_uri = contract.getServiceURI().call().await?;

            // Fetch the service JSON from the URI
            let service = fetch_service(&service_uri, ipfs_gateway).await?;

            Ok(service)
        }
        AnyChainConfig::Cosmos(_) => {
            unimplemented!()
        }
    }
}

// called at init and when a new service is added
async fn add_service_to_managers(
    service: Service,
    triggers: &TriggerManager,
    submissions: &SubmissionManager,
) -> Result<(), DispatcherError> {
    if let Err(err) = submissions.add_service(&service).await {
        tracing::error!("Error adding service to submission manager: {:?}", err);
        return Err(err.into());
    }

    for (id, workflow) in service.workflows {
        let trigger = TriggerConfig {
            service_id: service.id.clone(),
            workflow_id: id,
            trigger: workflow.trigger,
        };
        triggers.add_trigger(trigger)?;
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum DispatcherError {
    #[error("Service {0} already registered")]
    ServiceRegistered(ServiceID),

    #[error("Unknown Service {0}")]
    UnknownService(ServiceID),

    #[error("Invalid ID: {0}")]
    ID(#[from] IDError),

    #[error("DB: {0}")]
    DB(#[from] DBError),

    #[error("DB Storage: {0}")]
    DBStorage(#[from] redb::StorageError),

    #[error("DB: {0}")]
    CA(#[from] CAStorageError),

    #[error("Engine: {0}")]
    Engine(#[from] EngineError),

    #[error("Trigger: {0}")]
    Trigger(#[from] TriggerError),

    #[error("Submission: {0}")]
    Submission(#[from] SubmissionError),

    #[error("Registry error: {0}")]
    Registry(#[from] RegistryError),

    #[error("Chain config error: {0}")]
    ChainConfig(#[from] ChainConfigError),

    #[error("Registry cache path error: {0}")]
    RegistryCachePath(#[from] anyhow::Error),

    #[error("Alloy contract error: {0}")]
    AlloyContract(#[from] alloy_contract::Error),

    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("No registry domain provided in configuration")]
    NoRegistry,

    #[error("Unknown service digest: {0}")]
    UnknownDigest(Digest),

    #[error("Config error: {0}")]
    Config(String),
}
