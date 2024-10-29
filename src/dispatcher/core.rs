use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::mpsc};

use crate::apis::dispatcher::{DispatchManager, Service, Submit, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::submission::{ChainMessage, Submission};
use crate::apis::trigger::{TriggerData, TriggerError, TriggerManager, TriggerResult};
use crate::apis::{IDError, ID};

use crate::storage::db::{DBError, RedbStorage, Table, JSON};

pub struct Dispatcher<T: TriggerManager, E: Engine, S: Submission> {
    triggers: T,
    engine: E,
    submission: S,
    storage: RedbStorage,
}

impl<T: TriggerManager, E: Engine, S: Submission> Dispatcher<T, E, S> {
    pub fn new(
        triggers: T,
        engine: E,
        submission: S,
        file: impl AsRef<Path>,
    ) -> Result<Self, DispatcherError> {
        let storage = RedbStorage::new(file)?;
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
    /// If it is given a `rt` it will pass that runtime to triggers and submission, otherwise they will each create a new one.
    fn start(&self, rt: Option<Arc<Runtime>>) -> Result<(), DispatcherError> {
        let mut actions_in = self.triggers.start(rt.clone());
        let (msg_out, msg_in) = mpsc::channel::<ChainMessage>(1);
        self.submission.start(rt, msg_in);

        while let Some(action) = actions_in.blocking_recv() {
            // look up the proper workflow
            let service = self
                .storage
                .get(SERVICE_TABLE, action.service_id.as_ref())?
                .ok_or_else(|| DispatcherError::UnknownService(action.service_id.clone()))?
                .value();
            let workflow = service.workflows.get(&action.workflow_id).ok_or_else(|| {
                DispatcherError::UnknownWorkflow(
                    action.service_id.clone(),
                    action.workflow_id.clone(),
                )
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

                    if let Some(submit) = workflow.submit.as_ref() {
                        match submit {
                            Submit::VerifierTx {
                                hd_index,
                                verifier_addr,
                            } => {
                                let chain_msg = ChainMessage {
                                    service_id: action.service_id.clone(),
                                    workflow_id: action.workflow_id.clone(),
                                    task_id,
                                    wasm_result,
                                    hd_index: *hd_index,
                                    verifier_addr: verifier_addr.clone(),
                                };
                                msg_out.blocking_send(chain_msg).unwrap();
                            }
                        }
                    }
                }
            }
        }
        println!("Trigger channel closed, shutting down");
        Ok(())
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

    #[error("Engine: {0}")]
    Engine(#[from] EngineError),

    #[error("Trigger: {0}")]
    Trigger(#[from] TriggerError),
}
