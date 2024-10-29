use crate::apis::trigger::{TriggerAction, TriggerData, TriggerError, TriggerManager};
use crate::apis::ID;
use crate::context::AppContext;

use tokio::sync::mpsc;

// Annoying that TriggerAction cannot implement Clone (due to anyhow variant)
// So I need to store a function here rather than a simple element
#[derive(Clone)]
pub struct MockTriggerManager {
    triggers: Vec<TriggerAction>,
    error_on_start: bool,
    error_on_store: bool,
    // FIXME: store trigger data for proper list response
}

impl MockTriggerManager {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            triggers: vec![],
            error_on_start: false,
            error_on_store: false,
        }
    }

    pub fn with_actions(triggers: Vec<TriggerAction>) -> Self {
        Self {
            triggers,
            error_on_start: false,
            error_on_store: false,
        }
    }

    pub fn failing() -> Self {
        Self {
            triggers: vec![],
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
    fn start(&self, _ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        self.start_error()?;
        let (sender, receiver) = mpsc::channel(self.triggers.len() + 1);
        for t in self.triggers.clone() {
            let _ = sender.blocking_send(t);
        }
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
