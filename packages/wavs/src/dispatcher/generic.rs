use anyhow::Result;
use async_trait::async_trait;
use redb::ReadableTable;
use std::ops::Bound;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;
use wavs_types::{Digest, IDError, Service, ServiceID, TriggerAction, TriggerConfig};

use crate::apis::dispatcher::DispatchManager;
use crate::apis::engine::{Engine, EngineError};
use crate::apis::submission::{ChainMessage, Submission, SubmissionError};

use crate::apis::trigger::{TriggerError, TriggerManager};
use crate::engine::runner::EngineRunner;
use crate::AppContext;
use utils::storage::db::{DBError, RedbStorage, Table, JSON};
use utils::storage::CAStorageError;
use wasm_pkg_common::Error as RegistryError;

/// This should auto-derive clone if T, E, S: Clone
pub struct Dispatcher<T: TriggerManager, E: EngineRunner, S: Submission> {
    pub triggers: T,
    pub engine: E,
    pub submission: S,
    pub storage: Arc<RedbStorage>,
}

impl<T: TriggerManager, E: EngineRunner, S: Submission> Dispatcher<T, E, S> {
    pub fn new(
        triggers: T,
        engine: E,
        submission: S,
        db_storage_path: impl AsRef<Path>,
    ) -> Result<Self, DispatcherError> {
        let storage = Arc::new(RedbStorage::new(db_storage_path)?);

        Ok(Dispatcher {
            triggers,
            engine,
            submission,
            storage,
        })
    }
}

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

const TRIGGER_PIPELINE_SIZE: usize = 20;
const SUBMISSION_PIPELINE_SIZE: usize = 20;

#[async_trait]
impl<T: TriggerManager, E: EngineRunner, S: Submission> DispatchManager for Dispatcher<T, E, S> {
    type Error = DispatcherError;

    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    #[instrument(level = "debug", skip(self, ctx), fields(subsys = "Dispatcher"))]
    fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        // Trigger is pipeline start
        let mut actions_in = self.triggers.start(ctx.clone())?;
        // Next is the local (blocking) processing
        let (work_sender, work_receiver) =
            mpsc::channel::<(TriggerAction, Service)>(TRIGGER_PIPELINE_SIZE);
        let (wasi_result_sender, wasi_result_receiver) =
            mpsc::channel::<ChainMessage>(SUBMISSION_PIPELINE_SIZE);
        // Then the engine processing
        self.engine
            .start(ctx.clone(), work_receiver, wasi_result_sender);
        // And pipeline finishes with submission
        self.submission.start(ctx.clone(), wasi_result_receiver)?;

        // populate the initial triggers
        let initial_services = self.list_services(Bound::Unbounded, Bound::Unbounded)?;
        tracing::info!("Initializing {} services", initial_services.len());
        for service in initial_services {
            ctx.rt.block_on(async {
                add_service_to_managers(service, &self.triggers, &self.submission).await
            })?;
        }

        // since triggers listens to the async kill signal handler and closes the channel when
        // it is triggered, we don't need to jump through hoops here to make an async block to listen.
        // Just waiting for the channel to close is enough.

        // This reads the actions, extends them with the local service data, and passes
        // the combined info down to the EngineRunner to work.
        while let Some(action) = actions_in.blocking_recv() {
            let service = match self
                .storage
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
    fn store_component_bytes(&self, source: Vec<u8>) -> Result<Digest, Self::Error> {
        let digest = self.engine.engine().store_component_bytes(&source)?;
        Ok(digest)
    }
    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn list_component_digests(&self) -> Result<Vec<Digest>, Self::Error> {
        let digests = self.engine.engine().list_digests()?;

        Ok(digests)
    }

    async fn add_service(&self, service: Service) -> Result<(), Self::Error> {
        // persist it in storage if not there yet
        if self
            .storage
            .get(SERVICE_TABLE, service.id.as_ref())?
            .is_some()
        {
            return Err(DispatcherError::ServiceRegistered(service.id));
        }

        for component in service.components.values() {
            self.engine
                .engine()
                .store_component_from_source(&component.source)
                .await?;
        }

        self.storage
            .set(SERVICE_TABLE, service.id.as_ref(), &service)?;

        add_service_to_managers(service, &self.triggers, &self.submission).await?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn remove_service(&self, id: ServiceID) -> Result<(), Self::Error> {
        self.storage.remove(SERVICE_TABLE, id.as_ref())?;
        self.engine.engine().remove_storage(&id);
        self.triggers.remove_service(id.clone())?;
        self.submission.remove_service(id)?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn list_services(
        &self,
        bounds_start: Bound<&str>,
        bounds_end: Bound<&str>,
    ) -> Result<Vec<Service>, Self::Error> {
        let res = self
            .storage
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
}

// called at init and when a new service is added
async fn add_service_to_managers(
    service: Service,
    triggers: &impl TriggerManager,
    submissions: &impl Submission,
) -> Result<(), DispatcherError> {
    submissions.add_service(&service).await?;

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

    #[error("Registry cache path error: {0}")]
    RegistryCachePath(#[from] anyhow::Error),

    #[error("No registry domain provided in configuration")]
    NoRegistry,

    #[error("Unknown service digest: {0}")]
    UnknownDigest(Digest),
}

#[cfg(test)]
mod tests {
    use crate::{
        apis::submission::ChainMessage,
        engine::{
            identity::IdentityEngine,
            mock::MockEngine,
            runner::{MultiEngineRunner, SingleEngineRunner},
        },
        init_tracing_tests,
        submission::mock::{mock_event_id, mock_event_order, MockSubmission},
        test_utils::{
            address::{rand_address_eth, rand_event_eth},
            mock::BigSquare,
        },
        triggers::mock::{
            mock_eth_event_trigger, mock_eth_event_trigger_config, MockTriggerManagerVec,
        },
    };
    use wavs_types::{
        ChainName, Component, ComponentID, ComponentSource, Envelope, PacketRoute, ServiceConfig,
        ServiceID, ServiceManager, ServiceStatus, Submit, TriggerData, Workflow, WorkflowID,
    };

    use super::*;

    /// Ensure that some items pass end-to-end in simplest possible setup
    #[test]
    fn dispatcher_pipeline_happy_path() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();
        let payload = b"foobar";

        let action = TriggerAction {
            config: mock_eth_event_trigger_config("service1", "workflow1"),
            data: TriggerData::new_raw(payload),
        };
        let ctx = AppContext::new();

        let action_clone = action.clone();
        let dispatcher = Dispatcher::new(
            MockTriggerManagerVec::new().with_actions(vec![action_clone]),
            SingleEngineRunner::new(IdentityEngine),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register a service to handle this action
        let digest = Digest::new(b"wasm1");
        let component_id = ComponentID::new("component1").unwrap();
        let service_manager_addr = rand_address_eth();
        let service = Service {
            id: action.config.service_id.clone(),
            name: "My awesome service".to_string(),
            components: [(
                component_id.clone(),
                Component::new(ComponentSource::Digest(digest)),
            )]
            .into(),
            config: ServiceConfig::default(),
            workflows: [(
                action.config.workflow_id.clone(),
                Workflow {
                    component: component_id.clone(),
                    trigger: mock_eth_event_trigger(),
                    submit: Submit::eth_contract(
                        ChainName::new("eth").unwrap(),
                        service_manager_addr,
                        None,
                    ),
                    fuel_limit: None,
                    aggregator: None,
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Ethereum {
                chain_name: ChainName::new("eth").unwrap(),
                address: service_manager_addr,
            },
        };
        ctx.rt.block_on(async {
            dispatcher.add_service(service).await.unwrap();
        });

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        dispatcher.start(ctx).unwrap();

        // check that this event was properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(1).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 1);
        let expected = ChainMessage {
            packet_route: PacketRoute::new_trigger_config(&action.config),
            envelope: Envelope {
                eventId: mock_event_id().into(),
                ordering: mock_event_order().into(),
                payload: payload.into(),
            },
            submit: Submit::eth_contract(
                ChainName::new("eth").unwrap(),
                service_manager_addr,
                None,
            ),
        };
        assert_eq!(processed[0].envelope.payload, expected.envelope.payload);
    }

    /// Simulate running the square workflow but Function not WASI component
    #[test]
    fn dispatcher_big_square_mocked() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();

        // Prepare two actions to be squared
        let service_id = ServiceID::new("service1").unwrap();
        let workflow_id = WorkflowID::new("workflow1").unwrap();

        let contract_address = rand_address_eth();
        let actions = vec![
            TriggerAction {
                config: TriggerConfig::eth_contract_event(
                    &service_id,
                    &workflow_id,
                    contract_address,
                    ChainName::new("eth").unwrap(),
                    rand_event_eth(),
                )
                .unwrap(),
                data: TriggerData::new_raw(br#"{"x":3}"#),
            },
            TriggerAction {
                config: TriggerConfig::eth_contract_event(
                    &service_id,
                    &workflow_id,
                    contract_address,
                    ChainName::new("eth").unwrap(),
                    rand_event_eth(),
                )
                .unwrap(),
                data: TriggerData::new_raw(br#"{"x":21}"#),
            },
        ];

        let ctx = AppContext::new();
        // Set up the dispatcher
        let dispatcher = Dispatcher::new(
            MockTriggerManagerVec::new().with_actions(actions),
            SingleEngineRunner::new(MockEngine::new()),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register the BigSquare function on our known digest
        let digest = Digest::new(b"wasm1");
        dispatcher
            .engine
            .engine()
            .register(&digest.clone(), BigSquare);

        // Register a service to handle this action
        let component_id = ComponentID::new("component1").unwrap();

        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            components: [(
                component_id.clone(),
                Component::new(ComponentSource::Digest(digest)),
            )]
            .into(),
            config: ServiceConfig::default(),
            workflows: [(
                workflow_id.clone(),
                Workflow {
                    component: component_id.clone(),
                    trigger: mock_eth_event_trigger(),
                    submit: Submit::eth_contract(
                        ChainName::new("eth").unwrap(),
                        rand_address_eth(),
                        None,
                    ),
                    fuel_limit: None,
                    aggregator: None,
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Ethereum {
                chain_name: ChainName::new("eth").unwrap(),
                address: rand_address_eth(),
            },
        };
        ctx.rt.block_on(async {
            dispatcher.add_service(service).await.unwrap();
        });

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        dispatcher.start(ctx).unwrap();

        // check that the events were properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(2).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 2);

        // Check the payloads
        assert_eq!(&processed[0].envelope.payload.to_vec(), br#"{"y":9}"#);
        assert_eq!(&processed[1].envelope.payload.to_vec(), br#"{"y":441}"#);
    }

    /// Simulate big-square on a multi-threaded dispatcher
    /// TODO: don't copy this test, but refactor the above for reuse
    #[test]
    fn multi_dispatcher_big_square_mocked() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();

        // Prepare two actions to be squared
        let service_id = ServiceID::new("service1").unwrap();
        let workflow_id = WorkflowID::new("workflow1").unwrap();
        let contract_address = rand_address_eth();
        let actions = vec![
            TriggerAction {
                config: TriggerConfig::eth_contract_event(
                    &service_id,
                    &workflow_id,
                    contract_address,
                    ChainName::new("eth").unwrap(),
                    rand_event_eth(),
                )
                .unwrap(),
                data: TriggerData::new_raw(br#"{"x":3}"#),
            },
            TriggerAction {
                config: TriggerConfig::eth_contract_event(
                    &service_id,
                    &workflow_id,
                    contract_address,
                    ChainName::new("eth").unwrap(),
                    rand_event_eth(),
                )
                .unwrap(),
                data: TriggerData::new_raw(br#"{"x":21}"#),
            },
        ];

        // Set up the dispatcher
        let dispatcher = Dispatcher::new(
            MockTriggerManagerVec::new().with_actions(actions),
            MultiEngineRunner::new(MockEngine::new(), 4),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register the BigSquare function on our known digest
        let digest = Digest::new(b"wasm1");
        dispatcher
            .engine
            .engine()
            .register(&digest.clone(), BigSquare);

        // Register a service to handle this action
        let component_id = ComponentID::new("component1").unwrap();
        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            components: [(
                component_id.clone(),
                Component::new(ComponentSource::Digest(digest)),
            )]
            .into(),
            config: ServiceConfig::default(),
            workflows: [(
                workflow_id.clone(),
                Workflow {
                    component: component_id.clone(),
                    trigger: mock_eth_event_trigger(),
                    submit: Submit::eth_contract(
                        ChainName::new("eth").unwrap(),
                        rand_address_eth(),
                        None,
                    ),
                    fuel_limit: None,
                    aggregator: None,
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Ethereum {
                chain_name: ChainName::new("eth").unwrap(),
                address: rand_address_eth(),
            },
        };

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        let ctx = AppContext::new();
        ctx.rt.block_on(async {
            dispatcher.add_service(service).await.unwrap();
        });
        dispatcher.start(ctx).unwrap();

        // check that the events were properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(2).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 2);

        // Check the payloads
        assert_eq!(&processed[0].envelope.payload.to_vec(), br#"{"y":9}"#);
        assert_eq!(&processed[1].envelope.payload.to_vec(), br#"{"y":441}"#);
    }
}
