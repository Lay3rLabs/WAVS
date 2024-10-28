use std::path::Path;
use std::sync::RwLock;
use thiserror::Error;
use tokio::sync::mpsc::Receiver;

use crate::apis::dispatcher::{DispatchManager, Service, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager};
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
        while let Some(_action) = self.actions_in.write().unwrap().blocking_recv() {
            // TODO: look up the proper workflow

            // TODO: get the timestamp from trigger, don't invent it
            let _timestamp = 1234567890;

            // TODO: call the engine
            // self.engine.execute_queue(digest, request, timestamp)
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

    #[error("Invalid ID: {0}")]
    ID(#[from] IDError),

    #[error("DB: {0}")]
    DB(#[from] DBError),

    #[error("Engine: {0}")]
    Engine(#[from] EngineError),

    #[error("Trigger: {0}")]
    Trigger(#[from] TriggerError),
}
