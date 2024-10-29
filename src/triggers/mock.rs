use crate::{
    apis::trigger::{TriggerAction, TriggerError, TriggerManager},
    context::AppContext,
};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct MockTriggerManager {}

impl MockTriggerManager {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl TriggerManager for MockTriggerManager {
    fn start(
        &self,
        _ctx: AppContext,
    ) -> Result<mpsc::UnboundedReceiver<TriggerAction>, TriggerError> {
        todo!()
    }

    fn add_trigger(&self, _trigger: crate::apis::trigger::TriggerData) -> Result<(), TriggerError> {
        todo!()
    }

    fn remove_trigger(
        &self,
        _service_id: crate::apis::ID,
        _workflow_id: crate::apis::ID,
    ) -> Result<(), TriggerError> {
        todo!()
    }

    fn remove_service(&self, _service_id: crate::apis::ID) -> Result<(), TriggerError> {
        todo!()
    }

    fn list_triggers(
        &self,
        _service_id: crate::apis::ID,
    ) -> Result<Vec<crate::apis::trigger::TriggerData>, TriggerError> {
        todo!()
    }
}
