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
 * * When the workflow is created, it adds all relevant triggers to the TriggerManager
 * * It continually listens to new results from the TriggerManager, and executes them on the WasmEngine.
 * * When the WasmEngine has produced the result, it submits it to the verifier contract.
 *
 * The TriggerManager has it's own internal runtime and is meant to be able to handle a large number of
 * async network requests. These may be polling or event-driven (websockets), but there are expected to be quite
 * a few network calls and relatively little computation.
 *
 * The WasmEngine stores a large number of wasm components, indexed by their digest.
 * It should be able to quickly execute any of them, via a number of predefined wit component interfaces.
 * We do want to limit the number of wasmtime instances at once. For testing, a simple Mutex around the WasmEngine
 * should demo this. For real usage, we should use some internal threadpool like rayon at set a max number of
 * engines running at once. We may want to make this an async interface?
 *
 * Once the results are calculated, they need to be signed and submitted to the chain (or later to the aggregator).
 * We can do this in the operatator itself, or design a new subsystem for that. (Open to suggestions).
 *
 * I think the biggest question in my head is how to handle all these different runtimes and sync/async assumptions.
 * * Tokio channels is one way (which triggers use as it really matches this fan-in element well) - which allow each side to be either sync or async.
 * * Async code can call sync via `tokio::spawn_blocking`, but we may need some limit on how many such threads can be active at once
 *
 * Currently, I have a strong inclination to use sync code for:
 * * WasmEngine (it seems more stable)
 * * ReDB / KVStore (official recommendation is to wrap it with `tokio::block_in_place` or such if you need it async)
 *
 * And use async code for:
 * * TriggerManager
 * * HTTP Server
 *
 * I think the internal operation of the Dispatcher is my biggest question.
 * Along with how to organize the submission of results.
 * And then how to somehow throttle concurrent access to the WasmEngine.
 *
 ***/

/*

General execution workflow:

<Triggers> --Action--> <WasmEngine> --Result--> <Submission>

           mpsc channel               mpsc channel

Implementation: Actual pipeline is orchestrated by Dispatcher.
"Dispatcher" is like "event dispatcher" but also stores state and can reconstruct the other ones
Dispatcher should be quick, it has high-level system overview, just needs to delegate work to subsystems.


Idea 1

<Triggers> --TriggerAction-->     Dispatcher        --ChainMessage-->  <Submission>
                        - call WasmEngine
                        (call/response interface)


Idea 2

<Triggers> --TriggerAction-->  Dispatcher  --WasmRequest--> WasmEngine --WasmResult--> Dispatcher --ChainMessage-->  <Submission>
  async       (buffer?)      sync (select)                                         sync (select)

Trigger Action:
- (service, workflow) id
- task id (from queue)
- payload data

WasmRequest:
- (service, workflow id)
- task id
- payload data
- wasm digest

WasmResult:
- (service, workflow id)
- task id
- wasm result data

ChainMessage:
- (service, workflow id) ?? Do we need this anymore?
- task id
- wasm result data
- submit (hd_index, verifier_addr)

Dispatcher Thread 1 and 2 maintain some mapping by querying the workflow for the next step to execute.

HD Index must not be shared between different services.
For now assume all Submit in one service use the same HD Index.

Notes:

Dispatcher should allow multiple trigger actions to be run at the same time (some limit).

- WasmEngine can manage internal threadpool / concurrency limits
- Dispatcher has channel to WasmEngine, sends onshot channel with request to get result

* Look at backpressure
* Tracing, logging, metrics are important to monitor this pipeline

*/

/*

General management workflow
Sync calls on Dispatcher.

On load:
- Dispatcher loads all current state (list of registered services - workflows + triggers)
- Triggers wasm to refresh state if needed??
- Initializes all channels and subsystems (trigger, wasm engine, submission)
- Adds all triggers to trigger manager

On HTTP Request (local, from authorized agent):
- Update Dispatcher state
  - May store new wasm -> wasm engine (internal persistence)
  - May add/update triggers in trigger subsystem
  - Stores new services locally to manage workflows when triggers send actions

Management interface of Dispatcher may be somewhat slow, unlike the execution pipeline.
We also don't expect high-throughput here and could even limit to one management
operation at a time to simplify code for now.

HTTP server should call in `spawn_blocking` to avoid blocking the async runtime.
We can even use a mutex internally to ensure only one management call processed at a time.

Idea: HTTP server is outside of the Dispatcher and contains it as state once the Dispatcher
is properly initialized. It can then call into the Dispatcher to adjust running services.

- Management - set up workflows, add components
- Execution - run workflows, triggers -> wasm -> submit

*/
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
        self.submission_manager.remove_service(id.clone())?;

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

#[cfg(test)]
mod tests {
    use crate::{
        init_tracing_tests,
        test_utils::{
            address::{rand_address_cosmos, rand_address_evm},
            mock_app::MockE2ETestRunner,
            mock_engine::{SquareIn, SquareOut, COMPONENT_SQUARE},
            mock_submissions::wait_for_submission_messages,
            mock_trigger_manager::{mock_cosmos_event_trigger, mock_real_trigger_action},
        },
    };
    use alloy_sol_types::SolValue;
    use utils::test_utils::test_contracts::ISimpleSubmit::DataWithId;
    use wavs_types::{
        Aggregator, ChainName, Component, ComponentSource, EvmContractSubmission, ServiceID,
        ServiceManager, ServiceStatus, Submit, Workflow, WorkflowID,
    };

    use super::*;

    /// Simple test to check that the dispatcher can handle the full pipeline
    #[test]
    fn dispatcher_pipeline() {
        init_tracing_tests();

        let data_dir = tempfile::tempdir().unwrap();

        // Prepare two actions to be squared
        let service_id = ServiceID::new("service1").unwrap();
        let workflow_id = WorkflowID::new("workflow1").unwrap();
        let chain_name = "cosmos".to_string();

        let contract_address = rand_address_cosmos();
        let actions = vec![
            mock_real_trigger_action(
                &service_id,
                &workflow_id,
                &contract_address,
                &SquareIn::new(3),
                &chain_name,
            ),
            mock_real_trigger_action(
                &service_id,
                &workflow_id,
                &contract_address,
                &SquareIn::new(21),
                &chain_name,
            ),
        ];

        let ctx = AppContext::new();
        let dispatcher = Arc::new(MockE2ETestRunner::create_dispatcher(ctx.clone(), &data_dir));

        // Register the square component
        let digest = dispatcher
            .engine_manager
            .engine
            .store_component_bytes(COMPONENT_SQUARE)
            .unwrap();

        // Register a service to handle this action
        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            workflows: [(
                workflow_id.clone(),
                Workflow {
                    component: Component::new(ComponentSource::Digest(digest)),
                    trigger: mock_cosmos_event_trigger(),
                    submit: Submit::Aggregator {
                        url: "http://example.com/aggregator".to_string(),
                    },
                    aggregators: vec![Aggregator::Evm(EvmContractSubmission {
                        chain_name: chain_name.parse().unwrap(),
                        address: rand_address_evm(),
                        max_gas: None,
                    })],
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain_name: ChainName::new("evm").unwrap(),
                address: rand_address_evm(),
            },
        };

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            move || {
                dispatcher.start(ctx).unwrap();
            }
        });

        ctx.rt.block_on(async {
            dispatcher.add_service_direct(service).await.unwrap();
            dispatcher
                .trigger_manager
                .send_actions(actions)
                .await
                .unwrap();
        });

        // check that the events were properly handled and arrived at submission
        wait_for_submission_messages(&dispatcher.submission_manager, 2, None).unwrap();
        let processed = dispatcher.submission_manager.get_debug_packets();
        assert_eq!(processed.len(), 2);

        let payload_1: DataWithId = DataWithId::abi_decode(&processed[0].envelope.payload).unwrap();
        let data_1: SquareOut = serde_json::from_slice(&payload_1.data).unwrap();

        let payload_2: DataWithId = DataWithId::abi_decode(&processed[1].envelope.payload).unwrap();
        let data_2: SquareOut = serde_json::from_slice(&payload_2.data).unwrap();

        // Check the payloads
        assert_eq!(data_1, SquareOut::new(9));

        assert_eq!(data_2, SquareOut::new(441));
    }
}
