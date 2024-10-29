use std::sync::Arc;

use crate::apis::dispatcher::{Service, Submit, WasmSource};
use crate::apis::engine::Engine;
use crate::apis::submission::{ChainMessage, Submission};
use crate::apis::trigger::{TriggerAction, TriggerData, TriggerManager, TriggerResult};
use crate::apis::ID;
use crate::config::Config;
use crate::engine::WasmEngine;
use crate::storage::db::{Table, JSON};
use crate::storage::fs::FileStorage;
use crate::submission::core::CoreSubmission;
use crate::triggers::core::CoreTriggerManager;
use crate::{apis::dispatcher::DispatchManager, context::AppContext};

use super::generic::{Dispatcher, DispatcherError};

pub type CoreDispatcher =
    Dispatcher<CoreTriggerManager, Arc<WasmEngine<FileStorage>>, CoreSubmission>;

const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("services");

impl CoreDispatcher {
    pub fn new_core(config: &Config) -> Result<CoreDispatcher, DispatcherError> {
        let file_storage = FileStorage::new(config.data.join("ca"))?;

        let triggers = CoreTriggerManager::new(config)?;

        let engine = Arc::new(WasmEngine::new(file_storage));

        let submission = CoreSubmission::new(config)?;

        Self::new(triggers, engine, submission, config.data.join("db"))
    }
}

impl Clone for CoreDispatcher {
    fn clone(&self) -> Self {
        Self {
            triggers: self.triggers.clone(),
            engine: self.engine.clone(),
            submission: self.submission.clone(),
            storage: self.storage.clone(),
        }
    }
}

impl DispatchManager for CoreDispatcher {
    type Error = DispatcherError;

    /// This will run forever, taking the triggers, processing results, and sending them to submission to write.
    /// If it is given a `rt` it will pass that runtime to triggers and submission, otherwise they will each create a new one.
    fn start(&self, ctx: AppContext) -> Result<(), DispatcherError> {
        let mut actions_in = self.triggers.start(ctx.clone())?;
        let msgs_out = self.submission.start(ctx.clone())?;

        ctx.rt.clone().spawn({
            let mut kill_receiver = ctx.get_kill_receiver();
            let _self = self.clone();
            async move {
                tokio::select! {
                    _ = kill_receiver.recv() => {
                        tracing::info!("Dispatcher shutting down");
                    },
                    _ = async move {
                        while let Some(action) = actions_in.recv().await {
                            match _self.run_trigger(action) {
                                Err(e) => {
                                    tracing::error!("Error running trigger: {:?}", e);
                                },
                                Ok(Some(msg)) => {
                                    msgs_out.send(msg).await.unwrap();
                                },
                                Ok(None) => {
                                },
                            }
                        }
                    } => {
                    }
                }
            }
        });
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
