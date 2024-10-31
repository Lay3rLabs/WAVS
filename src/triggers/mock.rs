use std::time::Duration;

use crate::apis::trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager};
use crate::apis::ID;
use crate::context::AppContext;

use tokio::sync::mpsc;

// Annoying that TriggerAction cannot implement Clone (due to anyhow variant)
// So I need to store a function here rather than a simple element
#[derive(Clone)]
pub struct MockTriggerManager {
    triggers: Vec<TriggerAction>,
    delay: Duration,
    error_on_start: bool,
    error_on_store: bool,
    // FIXME: store trigger data for proper list response
}

impl MockTriggerManager {
    const DEFAULT_WAIT: Duration = Duration::from_millis(200);

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::with_actions(vec![])
    }

    pub fn with_actions(triggers: Vec<TriggerAction>) -> Self {
        Self::with_actions_and_wait(triggers, Self::DEFAULT_WAIT)
    }

    pub fn with_actions_and_wait(triggers: Vec<TriggerAction>, delay: Duration) -> Self {
        Self {
            triggers,
            delay,
            error_on_start: false,
            error_on_store: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            triggers: vec![],
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

impl TriggerManager for MockTriggerManager {
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        self.start_error()?;
        let (sender, receiver) = mpsc::channel(self.triggers.len() + 1);

        ctx.rt.clone().spawn({
            let triggers = self.triggers.clone();
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

    fn add_trigger(&self, _trigger: TriggerData) -> Result<(), TriggerError> {
        self.store_error()?;
        Ok(())
    }

    fn remove_trigger(&self, _service_id: ID, _workflow_id: ID) -> Result<(), TriggerError> {
        self.store_error()?;
        Ok(())
    }

    fn remove_service(&self, _service_id: ID) -> Result<(), TriggerError> {
        self.store_error()?;
        Ok(())
    }

    fn list_triggers(&self, _service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        self.store_error()?;
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use lavs_apis::id::TaskId;

    use crate::apis::{trigger::TriggerResult, Trigger};

    use super::*;

    #[test]
    fn mock_trigger_sends() {
        let actions = vec![
            TriggerAction {
                service_id: ID::new("service1").unwrap(),
                workflow_id: ID::new("workflow1").unwrap(),
                result: TriggerResult::Queue {
                    task_id: TaskId::new(2),
                    payload: "foobar".into(),
                },
            },
            TriggerAction {
                service_id: ID::new("service2").unwrap(),
                workflow_id: ID::new("workflow2").unwrap(),
                result: TriggerResult::Queue {
                    task_id: TaskId::new(4),
                    payload: "zoomba".into(),
                },
            },
        ];
        let triggers = MockTriggerManager::with_actions(actions.clone());
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
        let data = TriggerData {
            service_id: ID::new("abcd").unwrap(),
            workflow_id: ID::new("abcd").unwrap(),
            trigger: Trigger::Queue {
                task_queue_addr: "layer12345".into(),
                poll_interval: 5,
            },
        };
        triggers.add_trigger(data).unwrap();
    }

    #[test]
    fn mock_trigger_fails() {
        let triggers = MockTriggerManager::failing();
        // ensure start fails
        triggers.start(AppContext::new()).unwrap_err();

        // ensure store fails
        let data = TriggerData {
            service_id: ID::new("abcd").unwrap(),
            workflow_id: ID::new("abcd").unwrap(),
            trigger: Trigger::Queue {
                task_queue_addr: "layer12345".into(),
                poll_interval: 5,
            },
        };
        triggers.add_trigger(data).unwrap_err();
    }
}
