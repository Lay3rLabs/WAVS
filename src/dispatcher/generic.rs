use std::path::Path;
use std::sync::Arc;
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

impl<T, E, S> DispatchManager for Dispatcher<T, E, S>
where
    T: TriggerManager + Clone + 'static,
    E: Engine + Clone + 'static,
    S: Submission + Clone + 'static,
{
    type Error = DispatcherError;

    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        let mut actions_in = self.triggers.start(ctx.clone())?;
        let msgs_out = self.submission.start(ctx.clone())?;

        // we're only processing one item at a time for now, but in theory
        // this could eventually be something that feeds a threadpool
        // so let's give it a larger capacity to work with
        let (worker_tx, mut worker_rx) = tokio::sync::mpsc::channel(32);
        let _self = self.clone();

        // this will not hang because the kill switch will cause `worker_tx` to drop, thereby closing the channel
        let worker_handle = std::thread::spawn(move || match worker_rx.blocking_recv() {
            Some(action) => match _self.run_trigger(action) {
                Err(e) => {
                    tracing::error!("Error running trigger: {:?}", e);
                }
                Ok(Some(msg)) => {
                    if let Err(err) = msgs_out.blocking_send(msg) {
                        tracing::error!("Error submitting msg: {:?}", err);
                    }
                }
                Ok(None) => {}
            },
            None => {
                tracing::info!("no more work in dispatcher, channel closed");
            }
        });

        ctx.rt.clone().spawn({
            let mut kill_receiver = ctx.get_kill_receiver();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::info!("Dispatcher shutting down");
                    },
                    _ = async move {
                        while let Some(action) = actions_in.recv().await {
                            worker_tx.send(action).await.unwrap();
                        }
                    } => {
                    }
                }
            }
        });

        worker_handle.join().unwrap();

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
