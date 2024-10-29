use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::mpsc};
use std::sync::Arc;

use thiserror::Error;
use tokio::runtime::Runtime;

use crate::apis::dispatcher::{DispatchManager, Service, Submit, WasmSource};
use crate::apis::engine::{Engine, EngineError};
use crate::apis::submission::{ChainMessage, Submission};
use crate::apis::trigger::{TriggerData, TriggerError, TriggerManager, TriggerResult};
use crate::apis::trigger::{
    TriggerAction, TriggerData, TriggerError, TriggerManager, TriggerResult,
};
use crate::apis::{IDError, ID};

use crate::config::Config;
use crate::engine::WasmEngine;
use crate::storage::db::{DBError, RedbStorage, Table, JSON};
use crate::storage::fs::FileStorage;
use crate::storage::CAStorageError;
use crate::triggers::core::CoreTriggerManager;

pub type CoreDispatcher = Dispatcher<CoreTriggerManager, WasmEngine<FileStorage>, Submission>;

pub struct Dispatcher<T: TriggerManager, E: Engine, S: Submission> {
    triggers: T,
    engine: E,
    submission: S,
    storage: RedbStorage,
pub struct CoreDispatcher {
    triggers: CoreTriggerManager,
    engine: WasmEngine<FileStorage>,
    db_storage: RedbStorage,
    kill_sender: tokio::sync::broadcast::Sender<()>,
    pub async_runtime: Arc<Runtime>,
    pub config: Config,
}

impl<T: TriggerManager, E: Engine, S: Submission> Dispatcher<T, E, S> {
    pub fn new(
        triggers: T,
        engine: E,
        submission: S,
        db_storage_path: impl AsRef<Path>,
    ) -> Result<Self, DispatcherError> {
        let storage = RedbStorage::new(db_storage_path)?;
        Ok(Dispatcher {
            triggers,
            engine,
            submission,
            storage
        })
    }

impl CoreDispatcher {
    pub fn new(config: Config) -> Result<Self, DispatcherError> {
        println!(
            "{} -> {}",
            config.data.join("db").display(),
            config.data.join("db").exists()
        );

        let db_storage = RedbStorage::new(config.data.join("db"))?;
        let file_storage = FileStorage::new(config.data.join("ca"))?;

        let engine = WasmEngine::new(file_storage);

        let async_runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4) // TODO: make configurable?
                .enable_all()
                .build()
                .unwrap(),
        );

        let (kill_sender, kill_receiver) = tokio::sync::broadcast::channel(1);

        let triggers =
            CoreTriggerManager::new(config.clone(), async_runtime.clone(), kill_receiver)?;

        Ok(Self {
            triggers,
            engine,
            db_storage,
            async_runtime,
            config,
            kill_sender,
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
    /// This will run forever, taking the triggers and
    pub fn kill(&self) {
        let _ = self.kill_sender.send(());
    }

    /// This will run forever until killed
    /// taking the trigger actions and processing them as needed
    pub fn start(&self) -> Result<(), DispatcherError> {
        self.async_runtime.clone().block_on(async move {
            let mut kill_receiver = self.kill_receiver();
            let mut trigger_actions_receiver = self.triggers.start()?;

            tokio::select! {
                _ = async move {
                    while let Some(action) = trigger_actions_receiver.recv().await {
                        if let Err(e) = self.run_trigger(action) {
                            tracing::error!("Error running trigger: {:?}", e);
                        }
                    }
                } => {

                },
                _ = kill_receiver.recv() => {
                    tracing::info!("Dispatcher shutting down");
                },
            }

                    if let Some(submit) = workflow.submit.as_ref() {
                        match submit {
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
            Ok(())
        })

    fn run_trigger(&self, action: TriggerAction) -> Result<(), DispatcherError> {
        // look up the proper workflow
        let service = self
            .db_storage
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
                let _ = (task_id, wasm_result);
            }
        }
        Ok(())
    }
}

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

impl DispatchManager for CoreDispatcher {
    type Error = DispatcherError;

    fn config(&self) -> &Config {
        &self.config
    }

    fn kill_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.kill_sender.subscribe()
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
            .db_storage
            .get(SERVICE_TABLE, service.id.as_ref())?
            .is_some()
        {
            return Err(DispatcherError::ServiceRegistered(service.id));
        }
        self.db_storage
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
}
