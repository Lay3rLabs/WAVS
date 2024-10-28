use std::path::Path;
use std::sync::RwLock;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

use crate::apis::dispatcher::{DispatchManager, Service, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::trigger::{
    TriggerAction, TriggerData, TriggerError, TriggerManager, TriggerResult,
};
use crate::apis::{IDError, ID};

use crate::storage::db::{DBError, RedbStorage, Table, JSON};

pub struct Dispatcher<T: TriggerManager, E: Engine> {
    triggers: T,
    engine: E,
    storage: RedbStorage,
    actions_in: RwLock<Receiver<TriggerAction>>,
}

impl<T: TriggerManager, E: Engine> Dispatcher<T, E> {
    pub fn new(engine: E, file: impl AsRef<Path>) -> Result<Self, DispatcherError> {
        let storage = RedbStorage::new(file)?;
        let (triggers, channel) = T::create();
        let actions_in = RwLock::new(channel);
        Ok(Dispatcher {
            triggers,
            engine,
            storage,
            actions_in,
        })
    }

    /// This will run forever, taking the triggers and
    pub fn start(&self) -> Result<(), DispatcherError> {
        while let Some(action) = self.actions_in.write().unwrap().blocking_recv() {
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

                    // TODO: we need to sent these off to the submission engine
                    let _ = (task_id, wasm_result);
                }
            }
        }
        println!("Trigger channel closed, shutting down");
        Ok(())
    }
}

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

impl<T: TriggerManager, E: Engine> DispatchManager for Dispatcher<T, E> {
    type Error = DispatcherError;

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
