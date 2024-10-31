use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::apis::dispatcher::{DispatchManager, Service, Submit, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::submission::{ChainMessage, Submission, SubmissionError};
use crate::apis::trigger::{
    TriggerAction, TriggerData, TriggerError, TriggerManager, TriggerResult,
};
use crate::apis::{IDError, ID};

use crate::context::AppContext;
use crate::storage::db::{DBError, RedbStorage, Table, JSON};
use crate::storage::CAStorageError;

/// This should auto-derive clone if T, E, S: Clone
#[derive(Clone)]
pub struct Dispatcher<T: TriggerManager, E: Engine, S: Submission> {
    pub triggers: T,
    pub engine: E,
    pub submission: S,
    pub storage: Arc<RedbStorage>,
}

impl<T: TriggerManager, E: Engine, S: Submission> Dispatcher<T, E, S> {
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

impl<T: TriggerManager, E: Engine, S: Submission> DispatchManager for Dispatcher<T, E, S> {
    type Error = DispatcherError;

    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        let mut actions_in = self.triggers.start(ctx.clone())?;
        let msgs_out = self.submission.start(ctx.clone())?;

        // since triggers listens to the async kill signal handler and closes the channel when
        // it is triggered, we don't need to jump through hoops here to make an async block to listen.
        // Just waiting for the channel to close is enough.

        while let Some(action) = actions_in.blocking_recv() {
            match self.run_trigger(action) {
                Ok(Some(msg)) => {
                    tracing::info!("Ran action, got result to submit");
                    if let Err(err) = msgs_out.blocking_send(msg) {
                        tracing::error!("Error submitting msg: {:?}", err);
                    }
                }
                Ok(None) => {
                    tracing::info!("Ran action, no submission");
                }
                Err(e) => {
                    tracing::error!("Error running trigger: {:?}", e);
                }
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
        tracing::info!("no more work in dispatcher, channel closing");
        std::thread::sleep(Duration::from_millis(500));

        Ok(())
    }

    /// This is where the heavy lifting is done (at least for now, where self.engine.execute_queue happens in the same thread)
    /// effectively, it slows down the consumption of triggers and can inadvertendly cause the whole system to slow down
    /// TODO: optimize this, at the very least have wasm executions across a threadpool, and get real metrics to test assumptions about the performance
    fn run_trigger(&self, action: TriggerAction) -> Result<Option<ChainMessage>, DispatcherError> {
        // look up the proper workflow
        let service = self
            .storage
            .get(SERVICE_TABLE, action.service_id.as_ref())?
            .ok_or_else(|| DispatcherError::UnknownService(action.service_id.clone()))?
            .value();

        let workflow = service.workflows.get(&action.workflow_id).ok_or_else(|| {
            DispatcherError::UnknownWorkflow(action.service_id.clone(), action.workflow_id.clone())
        })?;

        let component = service
            .components
            .get(&workflow.component)
            .ok_or_else(|| DispatcherError::UnknownComponent(workflow.component.clone()))?;

        // TODO: we actually get other info, like permissions and apply in the execution
        let digest = component.wasm.clone();

        match action.result {
            TriggerResult::Queue { task_id, payload } => {
                // TODO: add the timestamp to the trigger, don't invent it
                let timestamp = 1234567890;
                let wasm_result = self.engine.execute_queue(digest, payload, timestamp)?;

                // TODO: we need to sent these off to the submission engine
                if let Some(Submit::VerifierTx {
                    hd_index,
                    verifier_addr,
                }) = workflow.submit.as_ref()
                {
                    Ok(Some(ChainMessage {
                        service_id: action.service_id.clone(),
                        workflow_id: action.workflow_id.clone(),
                        task_id,
                        wasm_result,
                        hd_index: *hd_index,
                        verifier_addr: verifier_addr.clone(),
                    }))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn store_component(&self, source: WasmSource) -> Result<crate::Digest, Self::Error> {
        let bytecode = match source {
            WasmSource::Bytecode(code) => code,
            _ => todo!(),
        };
        let digest = self.engine.store_wasm(&bytecode)?;
        Ok(digest)
    }

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

        // go through and add the triggers to the table
        for (id, workflow) in service.workflows {
            let trigger = TriggerData {
                service_id: service.id.clone(),
                workflow_id: id,
                trigger: workflow.trigger,
            };
            self.triggers.add_trigger(trigger)?;
        }

        Ok(())
    }

    fn remove_service(&self, _id: ID) -> Result<(), Self::Error> {
        // TODO: remove it from storage
        // TODO: remove all triggers
        todo!()
    }

    fn list_services(&self) -> Result<Vec<Service>, Self::Error> {
        // TODO: we need to list all keys of the storage (range and range_keys)
        todo!()
    }
}

#[derive(Error, Debug)]
pub enum DispatcherError {
    #[error("Service {0} already registered")]
    ServiceRegistered(ID),

    #[error("Unknown Service {0}")]
    UnknownService(ID),

    #[error("Unknown Workflow {0} / {1}")]
    UnknownWorkflow(ID, ID),

    #[error("Unknown Component {0}")]
    UnknownComponent(ID),

    #[error("Invalid ID: {0}")]
    ID(#[from] IDError),

    #[error("DB: {0}")]
    DB(#[from] DBError),

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
            dispatcher::{Component, ServiceStatus},
            Trigger,
        },
        engine::{
            identity::IdentityEngine,
            mock::{Function, MockEngine},
        },
        init_tracing_tests,
        submission::mock::MockSubmission,
        triggers::mock::MockTriggerManager,
        Digest,
    };
    use serde::{Deserialize, Serialize};

    use super::*;

    /// Ensure that some items pass end-to-end in simplest possible setup
    #[test]
    fn dispatcher_pipeline_happy_path() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();
        let task_id = TaskId::new(2);
        let payload = b"foobar";

        let action = TriggerAction {
            service_id: ID::new("service1").unwrap(),
            workflow_id: ID::new("workflow1").unwrap(),
            result: TriggerResult::queue(task_id, payload),
        };

        let dispatcher = Dispatcher::new(
            MockTriggerManager::with_actions(vec![action.clone()]),
            IdentityEngine::new(),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register a service to handle this action
        let digest = Digest::new(b"wasm1");
        let component_id = ID::new("component1").unwrap();
        let hd_index = 2;
        let verifier_addr = "layer1verifier";
        let service = Service {
            id: action.service_id.clone(),
            name: "My awesome service".to_string(),
            components: [(component_id.clone(), Component::new(&digest))].into(),
            workflows: [(
                action.workflow_id.clone(),
                crate::apis::dispatcher::Workflow {
                    component: component_id.clone(),
                    trigger: Trigger::queue("some-task", 5),
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

        // check that this event was properly handled and arrived at submission
        dispatcher.submission.wait_for_messages(1).unwrap();
        let processed = dispatcher.submission.received();
        assert_eq!(processed.len(), 1);
        let expected = ChainMessage {
            service_id: action.service_id.clone(),
            workflow_id: action.workflow_id.clone(),
            task_id,
            wasm_result: payload.into(),
            hd_index,
            verifier_addr: verifier_addr.to_string(),
        };
        assert_eq!(processed[0], expected);
    }

    struct BigSquare;

    #[derive(Deserialize, Serialize)]
    struct SquareIn {
        pub x: u64,
    }

    #[derive(Deserialize, Serialize)]
    struct SquareOut {
        pub y: u64,
    }

    impl Function for BigSquare {
        fn execute(&self, request: Vec<u8>, _timestamp: u64) -> Result<Vec<u8>, EngineError> {
            let SquareIn { x } = serde_json::from_slice(&request).unwrap();
            let output = SquareOut { y: x * x };
            Ok(serde_json::to_vec(&output).unwrap())
        }
    }

    /// Simulate running the square workflow but Function not WASI component
    #[test]
    fn dispatcher_big_square_mocked() {
        init_tracing_tests();

        let db_file = tempfile::NamedTempFile::new().unwrap();

        // Prepare two actions to be squared
        let service_id = ID::new("service1").unwrap();
        let workflow_id = ID::new("workflow1").unwrap();
        let actions = vec![
            TriggerAction {
                service_id: service_id.clone(),
                workflow_id: workflow_id.clone(),
                result: TriggerResult::queue(TaskId::new(1), br#"{"x":3}"#),
            },
            TriggerAction {
                service_id: service_id.clone(),
                workflow_id: workflow_id.clone(),
                result: TriggerResult::queue(TaskId::new(2), br#"{"x":21}"#),
            },
        ];

        // Set up the dispatcher
        let dispatcher = Dispatcher::new(
            MockTriggerManager::with_actions(actions),
            MockEngine::new(),
            MockSubmission::new(),
            db_file.as_ref(),
        )
        .unwrap();

        // Register the BigSquare function on our known digest
        let digest = Digest::new(b"wasm1");
        dispatcher.engine.register(&digest, BigSquare);

        // Register a service to handle this action
        let component_id = ID::new("component1").unwrap();
        let hd_index = 2;
        let verifier_addr = "layer1verifier";
        let service = Service {
            id: service_id.clone(),
            name: "Big Square AVS".to_string(),
            components: [(component_id.clone(), Component::new(&digest))].into(),
            workflows: [(
                workflow_id.clone(),
                crate::apis::dispatcher::Workflow {
                    component: component_id.clone(),
                    trigger: Trigger::queue("some-task", 5),
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
