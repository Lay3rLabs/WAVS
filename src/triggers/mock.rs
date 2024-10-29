use crate::apis::trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager};
use crate::apis::ID;
use crate::context::AppContext;

use tokio::sync::mpsc;

#[derive(Clone)]
pub struct MockTriggerManager {
    triggers: Vec<TriggerAction>, // TODO: add some error conditions

                                  // FIXME: store trigger data for proper list response
}

impl MockTriggerManager {
    pub fn new(triggers: Vec<TriggerAction>) -> Self {
        Self { triggers }
    }
}

impl TriggerManager for MockTriggerManager {
    fn start(&self, _ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        let (sender, receiver) = mpsc::channel(self.triggers.len() + 1);
        for t in self.triggers.clone() {
            sender.blocking_send(t);
        }
        Ok(receiver)
    }

    fn add_trigger(&self, _trigger: TriggerData) -> Result<(), TriggerError> {
        Ok(())
    }

    fn remove_trigger(&self, _service_id: ID, _workflow_id: ID) -> Result<(), TriggerError> {
        Ok(())
    }

    fn remove_service(&self, _service_id: ID) -> Result<(), TriggerError> {
        Ok(())
    }

    fn list_triggers(&self, _service_id: ID) -> Result<Vec<TriggerData>, TriggerError> {
        Ok(vec![])
    }
}
