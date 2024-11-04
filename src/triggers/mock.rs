use std::sync::atomic::AtomicU64;
use std::sync::{Mutex, RwLock};
use std::time::Duration;

use crate::apis::trigger::{
    TriggerAction, TriggerData, TriggerError, TriggerManager, TriggerResult,
};
use crate::apis::{IDError, ID};
use crate::context::AppContext;

use lavs_apis::id::TaskId;
use layer_climb::prelude::Address;
use serde::Serialize;
use tokio::sync::mpsc;

pub struct MockTriggerManagerVec {
    triggers: RwLock<Vec<TriggerAction>>,
    delay: Duration,
    error_on_start: bool,
    error_on_store: bool,
    // FIXME: store trigger data for proper list response
}

impl MockTriggerManagerVec {
    const DEFAULT_WAIT: Duration = Duration::from_millis(200);

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            triggers: RwLock::new(Vec::new()),
            delay: Self::DEFAULT_WAIT,
            error_on_start: false,
            error_on_store: false,
        }
    }

    pub fn with_actions(mut self, triggers: Vec<TriggerAction>) -> Self {
        self.triggers = RwLock::new(triggers);
        self
    }

    pub fn with_actions_and_wait(mut self, triggers: Vec<TriggerAction>, delay: Duration) -> Self {
        self.triggers = RwLock::new(triggers);
        self.delay = delay;
        self
    }

    pub fn failing() -> Self {
        Self {
            triggers: RwLock::new(vec![]),
            delay: Self::DEFAULT_WAIT,
            error_on_start: true,
            error_on_store: true,
        }
    }

    fn start_error(&self) -> Result<(), TriggerError> {
        match self.error_on_start {
            true => Err(TriggerError::NoSuchService(ID::new("cant-start").unwrap())),
            false => Ok(()),
        }
    }

    fn store_error(&self) -> Result<(), TriggerError> {
        match self.error_on_store {
            true => Err(TriggerError::NoSuchService(ID::new("cant-store").unwrap())),
            false => Ok(()),
        }
    }
}

impl TriggerManager for MockTriggerManagerVec {
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        self.start_error()?;

        let triggers:Vec<TriggerAction> = self.triggers.write().unwrap().drain(..).collect();

        let (sender, receiver) = mpsc::channel(triggers.len() + 1);

        ctx.rt.clone().spawn({
            let delay = self.delay;
            async move {
                for t in triggers {
                    tokio::time::sleep(delay).await;
                    sender.send(t).await.unwrap();
                }
            }
        });
        Ok(receiver)
    }

    fn add_trigger(&self, trigger: TriggerData) -> Result<(), TriggerError> {
        self.store_error()?;

        let mut latest_task_id = 0;
        for t in self.triggers.read().unwrap().iter() {
            let TriggerResult::Queue { task_id, .. } = &t.result;
            latest_task_id = latest_task_id.max(task_id.u64());
        }

        self.triggers
            .write()
            .unwrap()
            .push(TriggerAction {
                trigger,
                result: TriggerResult::Queue {
                    task_id: TaskId::new(latest_task_id + 1),
                    payload: b"mock".to_vec(),
                },
            });
        Ok(())
    }

    fn remove_trigger(&self, service_id: ID, workflow_id: ID) -> Result<(), TriggerError> {
        self.store_error()?;

        self
            .triggers
            .write()
            .unwrap()
            .retain(|t| t.trigger.service_id != service_id && t.trigger.workflow_id != workflow_id);
        Ok(())
    }

    fn remove_service(&self, service_id: ID) -> Result<(), TriggerError> {
        self.store_error()?;

        self
            .triggers
            .write()
            .unwrap()
            .retain(|t| t.trigger.service_id != service_id);

        Ok(())
    }

    fn list_triggers(&self, service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        self.store_error()?;

        self.triggers
            .read()
            .unwrap()
            .iter()
            .filter(|t| t.trigger.service_id == service_id)
            .map(|t| Ok(t.trigger.clone()))
            .collect()
    }
}

// This mock is currently only used in mock_e2e.rs
// it doesn't have the same coverage in unit tests here as MockTriggerManager
pub struct MockTriggerManagerChannel {
    trigger_count: AtomicU64,
    sender: mpsc::Sender<TriggerAction>,
    receiver: Mutex<Option<mpsc::Receiver<TriggerAction>>>,
    trigger_datas: Mutex<Vec<TriggerData>>,
}

impl MockTriggerManagerChannel {
    #[allow(clippy::new_without_default)]
    pub fn new(channel_bound: usize) -> Self {
        let (sender, receiver) = mpsc::channel(channel_bound);

        Self {
            trigger_count: AtomicU64::new(1),
            receiver: Mutex::new(Some(receiver)),
            sender,
            trigger_datas: Mutex::new(Vec::new()),
        }
    }

    pub async fn send_trigger(
        &self,
        service_id: impl TryInto<ID, Error = IDError>,
        workflow_id: impl TryInto<ID, Error = IDError>,
        task_queue_addr: &Address,
        data: &impl Serialize,
    ) {
        let task_id = TaskId::new(
            self.trigger_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        self.sender
            .send(TriggerAction {
                trigger: TriggerData::queue(
                    service_id,
                    workflow_id,
                    &task_queue_addr.to_string(),
                    5,
                )
                .unwrap(),
                result: TriggerResult::queue(
                    task_id,
                    serde_json::to_string(data).unwrap().as_bytes(),
                ),
            })
            .await
            .unwrap();
    }
}

impl TriggerManager for MockTriggerManagerChannel {
    fn start(&self, _ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        let receiver = self.receiver.lock().unwrap().take().unwrap();
        Ok(receiver)
    }

    fn add_trigger(&self, trigger: TriggerData) -> Result<(), TriggerError> {
        self.trigger_datas.lock().unwrap().push(trigger);
        Ok(())
    }

    fn remove_trigger(&self, service_id: ID, workflow_id: ID) -> Result<(), TriggerError> {
        self.trigger_datas
            .lock()
            .unwrap()
            .retain(|t| t.service_id != service_id && t.workflow_id != workflow_id);
        Ok(())
    }

    fn remove_service(&self, service_id: ID) -> Result<(), TriggerError> {
        self.trigger_datas
            .lock()
            .unwrap()
            .retain(|t| t.service_id != service_id);
        Ok(())
    }

    fn list_triggers(&self, service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        let triggers = self.trigger_datas.lock().unwrap();
        let triggers = triggers
            .iter()
            .filter(|t| t.service_id == service_id)
            .cloned()
            .collect();
        Ok(triggers)
    }
}

#[cfg(test)]
mod tests {
    use lavs_apis::id::TaskId;

    use crate::apis::trigger::TriggerResult;

    use super::*;

    #[test]
    fn mock_trigger_sends() {
        let actions = vec![
            TriggerAction {
                trigger: TriggerData::queue("service1", "workflow1", "layer12345", 5).unwrap(),
                result: TriggerResult::Queue {
                    task_id: TaskId::new(2),
                    payload: "foobar".into(),
                },
            },
            TriggerAction {
                trigger: TriggerData::queue("service2", "workflow2", "layer12345", 5).unwrap(),
                result: TriggerResult::Queue {
                    task_id: TaskId::new(4),
                    payload: "zoomba".into(),
                },
            },
        ];
        let triggers = MockTriggerManagerVec::new().with_actions(actions.clone());
        let ctx = AppContext::new();
        let mut flow = triggers.start(ctx.clone()).unwrap();

        // read the triggers
        let first = flow.blocking_recv().unwrap();
        assert_eq!(&first, &actions[0]);
        let second = flow.blocking_recv().unwrap();
        assert_eq!(&second, &actions[1]);

        // channel is closed
        assert!(flow.blocking_recv().is_none());

        // add trigger works
        let data = TriggerData::queue("abcd", "abcd", "layer12345", 5).unwrap();
        triggers.add_trigger(data).unwrap();
    }

    #[test]
    fn mock_trigger_fails() {
        let triggers = MockTriggerManagerVec::failing();
        // ensure start fails
        triggers.start(AppContext::new()).unwrap_err();

        // ensure store fails
        let data = TriggerData::queue("abcd", "abcd", "layer12345", 5).unwrap();
        triggers.add_trigger(data).unwrap_err();
    }
}
