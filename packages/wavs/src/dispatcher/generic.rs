use redb::ReadableTable;
use std::ops::Bound;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;

use crate::apis::dispatcher::{DispatchManager, Service, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::submission::{Submission, SubmissionError};
use crate::apis::trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager};
use crate::apis::{IDError, ID};

use crate::context::AppContext;
use crate::engine::runner::EngineRunner;
use crate::storage::db::{DBError, RedbStorage, Table, JSON};
use crate::storage::CAStorageError;

/// This should auto-derive clone if T, E, S: Clone
#[derive(Clone)]
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
        // Then the engine processing
        let msgs_out = self.engine.start(ctx.clone(), work_receiver)?;
        // And pipeline finishes with submission
        self.submission.start(ctx.clone(), msgs_out)?;

        // populate the initial triggers
        let initial_services = self.list_services(Bound::Unbounded, Bound::Unbounded)?;
        tracing::info!("Initializing {} services", initial_services.len());
        for service in initial_services {
            add_service_to_trigger_manager(service, &self.triggers)?;
        }

        // since triggers listens to the async kill signal handler and closes the channel when
        // it is triggered, we don't need to jump through hoops here to make an async block to listen.
        // Just waiting for the channel to close is enough.

        // This reads the actions, extends them with the local service data, and passes
        // the combined info down to the EngineRunner to work.
        while let Some(action) = actions_in.blocking_recv() {
            let service = match self
                .storage
                .get(SERVICE_TABLE, action.trigger.service_id.as_ref())?
            {
                Some(service) => service.value(),
                None => {
                    let err = DispatcherError::UnknownService(action.trigger.service_id.clone());
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
    fn run_trigger(
        &self,
        action: TriggerAction,
    ) -> Result<Option<crate::apis::submission::ChainMessage>, Self::Error> {
        let service = self
            .storage
            .get(SERVICE_TABLE, action.trigger.service_id.as_ref())?
            .ok_or(DispatcherError::UnknownService(
                action.trigger.service_id.clone(),
            ))?
            .value();

        Ok(self.engine.run_trigger(action, service)?)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn store_component(&self, source: WasmSource) -> Result<crate::Digest, Self::Error> {
        let bytecode = match source {
            WasmSource::Bytecode(code) => code,
            _ => todo!(),
        };
        let digest = self.engine.engine().store_wasm(&bytecode)?;
        Ok(digest)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn list_component_digests(&self) -> Result<Vec<crate::Digest>, Self::Error> {
        let digests = self.engine.engine().list_digests()?;

        Ok(digests)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn add_service(&self, service: Service) -> Result<(), Self::Error> {
        // persist it in storage if not there yet
        if self
            .storage
            .get(SERVICE_TABLE, service.id.as_ref())?
            .is_some()
        {
            return Err(DispatcherError::ServiceRegistered(service.id));
        }
        self.storage
            .set(SERVICE_TABLE, service.id.as_ref(), &service)?;

        add_service_to_trigger_manager(service, &self.triggers)?;

        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "Dispatcher"))]
    fn remove_service(&self, id: ID) -> Result<(), Self::Error> {
        self.storage.remove(SERVICE_TABLE, id.as_ref())?;
        self.triggers.remove_service(id)?;

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
fn add_service_to_trigger_manager(
    service: Service,
    triggers: &impl TriggerManager,
) -> Result<(), DispatcherError> {
    for (id, workflow) in service.workflows {
        let trigger = TriggerData {
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
    ServiceRegistered(ID),

    #[error("Unknown Service {0}")]
    UnknownService(ID),

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
}

#[cfg(test)]
mod tests {
    use lavs_apis::id::TaskId;

    use crate::{
        apis::{
            dispatcher::{Component, ServiceStatus, Submit},
            submission::ChainMessage,
            trigger::TriggerResult,
            ChainKind, Trigger,
        },
        engine::{
            identity::IdentityEngine,
            mock::MockEngine,
            runner::{MultiEngineRunner, SingleEngineRunner},
        },
        init_tracing_tests,
        submission::mock::MockSubmission,
        test_utils::{address::rand_address, mock::BigSquare},
        triggers::mock::MockTriggerManagerVec,
        Digest,
    };

    use super::*;

    /// Ensure that some items pass end-to-end in simplest possible setup
    #[test]
    fn dispatcher_pipeline_happy_path() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();
        let task_id = TaskId::new(2);
        let payload = b"foobar";

        let action = TriggerAction {
            chain_kind: ChainKind::Ethereum,
            trigger: TriggerData::queue("service1", "workflow1", rand_address(), 5).unwrap(),
            result: TriggerResult::queue(task_id, payload),
        };

        let dispatcher = Dispatcher::new(
            MockTriggerManagerVec::new().with_actions(vec![action.clone()]),
            SingleEngineRunner::new(IdentityEngine),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register a service to handle this action
        let digest = Digest::new(b"wasm1");
        let component_id = ID::new("component1").unwrap();
        let hd_index = 2;
        let verifier_addr = rand_address();
        let service = Service {
            id: action.trigger.service_id.clone(),
            name: "My awesome service".to_string(),
            components: [(component_id.clone(), Component::new(&digest))].into(),
            workflows: [(
                action.trigger.workflow_id.clone(),
                crate::apis::dispatcher::Workflow {
                    component: component_id.clone(),
                    trigger: Trigger::queue(rand_address(), 5),
                    submit: Some(Submit::verifier_tx(hd_index, verifier_addr.clone())),
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            testable: false,
        };
        dispatcher.add_service(service).unwrap();

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        let ctx = AppContext::new();
        dispatcher.start(ctx).unwrap();

        // check that this event was properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(1).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 1);
        let expected = ChainMessage {
            trigger_data: action.trigger,
            task_id,
            wasm_result: payload.into(),
            hd_index,
            verifier_addr,
        };
        assert_eq!(processed[0], expected);
    }

    /// Simulate running the square workflow but Function not WASI component
    #[test]
    fn dispatcher_big_square_mocked() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();

        // Prepare two actions to be squared
        let service_id = ID::new("service1").unwrap();
        let workflow_id = ID::new("workflow1").unwrap();

        let task_queue_address = rand_address();
        let actions = vec![
            TriggerAction {
                chain_kind: ChainKind::Ethereum,
                trigger: TriggerData::queue(
                    &service_id,
                    &workflow_id,
                    task_queue_address.clone(),
                    5,
                )
                .unwrap(),
                result: TriggerResult::queue(TaskId::new(1), br#"{"x":3}"#),
            },
            TriggerAction {
                chain_kind: ChainKind::Ethereum,
                trigger: TriggerData::queue(&service_id, &workflow_id, task_queue_address, 5)
                    .unwrap(),
                result: TriggerResult::queue(TaskId::new(2), br#"{"x":21}"#),
            },
        ];

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
        dispatcher.engine.engine().register(&digest, BigSquare);

        // Register a service to handle this action
        let component_id = ID::new("component1").unwrap();
        let hd_index = 2;
        let verifier_addr = rand_address();
        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            components: [(component_id.clone(), Component::new(&digest))].into(),
            workflows: [(
                workflow_id.clone(),
                crate::apis::dispatcher::Workflow {
                    component: component_id.clone(),
                    trigger: Trigger::queue(rand_address(), 5),
                    submit: Some(Submit::verifier_tx(hd_index, verifier_addr)),
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            testable: false,
        };
        dispatcher.add_service(service).unwrap();

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        let ctx = AppContext::new();
        dispatcher.start(ctx).unwrap();

        // check that the events were properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(2).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 2);

        // Check the task_id and payloads
        assert_eq!(processed[0].task_id, TaskId::new(1));
        assert_eq!(&processed[0].wasm_result, br#"{"y":9}"#);
        assert_eq!(processed[1].task_id, TaskId::new(2));
        assert_eq!(&processed[1].wasm_result, br#"{"y":441}"#);
    }

    /// Simulate big-square on a multi-threaded dispatcher
    /// TODO: don't copy this test, but refactor the above for reuse
    #[test]
    fn multi_dispatcher_big_square_mocked() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();

        // Prepare two actions to be squared
        let service_id = ID::new("service1").unwrap();
        let workflow_id = ID::new("workflow1").unwrap();
        let task_queue_address = rand_address();
        let actions = vec![
            TriggerAction {
                chain_kind: ChainKind::Ethereum,
                trigger: TriggerData::queue(
                    &service_id,
                    &workflow_id,
                    task_queue_address.clone(),
                    5,
                )
                .unwrap(),
                result: TriggerResult::queue(TaskId::new(1), br#"{"x":3}"#),
            },
            TriggerAction {
                chain_kind: ChainKind::Ethereum,
                trigger: TriggerData::queue(&service_id, &workflow_id, task_queue_address, 5)
                    .unwrap(),
                result: TriggerResult::queue(TaskId::new(2), br#"{"x":21}"#),
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
        dispatcher.engine.engine().register(&digest, BigSquare);

        // Register a service to handle this action
        let component_id = ID::new("component1").unwrap();
        let hd_index = 2;
        let verifier_addr = rand_address();
        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            components: [(component_id.clone(), Component::new(&digest))].into(),
            workflows: [(
                workflow_id.clone(),
                crate::apis::dispatcher::Workflow {
                    component: component_id.clone(),
                    trigger: Trigger::queue(rand_address(), 5),
                    submit: Some(Submit::verifier_tx(hd_index, verifier_addr)),
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            testable: false,
        };
        dispatcher.add_service(service).unwrap();

        // runs "forever" until the channel is closed, which should happen as soon as the one action is sent
        let ctx = AppContext::new();
        dispatcher.start(ctx).unwrap();

        // check that the events were properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(2).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 2);

        // Check the task_id and payloads
        assert_eq!(processed[0].task_id, TaskId::new(1));
        assert_eq!(&processed[0].wasm_result, br#"{"y":9}"#);
        assert_eq!(processed[1].task_id, TaskId::new(2));
        assert_eq!(&processed[1].wasm_result, br#"{"y":441}"#);
    }
}
