use std::sync::RwLock;
use std::time::Duration;

use crate::test_utils::address::{
    rand_address_cosmos, rand_address_evm, rand_event_cosmos, rand_event_evm,
};
use crate::trigger_manager::error::TriggerError;

use tokio::sync::mpsc;
use utils::context::AppContext;
use wavs_types::{
    ChainName, IDError, ServiceID, Trigger, TriggerAction, TriggerConfig, TriggerData, WorkflowID,
};

pub fn mock_evm_event_trigger_config(
    service_id: impl TryInto<ServiceID, Error = IDError>,
    workflow_id: impl TryInto<WorkflowID, Error = IDError>,
) -> TriggerConfig {
    TriggerConfig::evm_contract_event(
        service_id,
        workflow_id,
        rand_address_evm(),
        ChainName::new("evm").unwrap(),
        rand_event_evm(),
    )
    .unwrap()
}

pub fn mock_cosmos_event_trigger_config(
    service_id: impl TryInto<ServiceID, Error = IDError>,
    workflow_id: impl TryInto<WorkflowID, Error = IDError>,
) -> TriggerConfig {
    TriggerConfig::cosmos_contract_event(
        service_id,
        workflow_id,
        rand_address_cosmos(),
        ChainName::new("cosmos").unwrap(),
        rand_event_cosmos(),
    )
    .unwrap()
}

pub fn mock_evm_event_trigger() -> Trigger {
    Trigger::evm_contract_event(
        rand_address_evm(),
        ChainName::new("evm").unwrap(),
        rand_event_evm(),
    )
}

pub fn mock_cosmos_event_trigger() -> Trigger {
    Trigger::cosmos_contract_event(
        rand_address_cosmos(),
        ChainName::new("cosmos").unwrap(),
        rand_event_cosmos(),
    )
}

pub fn mock_cosmos_event_trigger_data(trigger_id: u64, data: impl AsRef<[u8]>) -> TriggerData {
    TriggerData::CosmosContractEvent {
        contract_address: rand_address_cosmos(),
        chain_name: ChainName::new("layer").unwrap(),
        // matches example_cosmos_client::NewMessageEvent
        event: cosmwasm_std::Event::new("new-message")
            .add_attribute("id", trigger_id.to_string())
            .add_attribute("data", const_hex::encode(data.as_ref())),
        block_height: 0,
    }
}

pub fn get_mock_trigger_data(trigger_data: &TriggerData) -> Vec<u8> {
    match trigger_data {
        TriggerData::Raw(data) => data.to_vec(),
        _ => panic!("mocks need raw trigger data"),
    }
}

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
            true => Err(TriggerError::NoSuchService(
                ServiceID::new("cant-start").unwrap(),
            )),
            false => Ok(()),
        }
    }

    fn store_error(&self) -> Result<(), TriggerError> {
        match self.error_on_store {
            true => Err(TriggerError::NoSuchService(
                ServiceID::new("cant-store").unwrap(),
            )),
            false => Ok(()),
        }
    }
}

impl MockTriggerManagerVec {
    pub fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        self.start_error()?;

        let triggers: Vec<TriggerAction> = self.triggers.write().unwrap().drain(..).collect();

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

    pub fn add_trigger(&self, _trigger: TriggerConfig) -> Result<(), TriggerError> {
        self.store_error()?;

        // MockTriggerManagerVec doesn't allow adding new triggers, since they need their data too
        Ok(())
    }

    pub fn remove_trigger(
        &self,
        service_id: ServiceID,
        workflow_id: WorkflowID,
    ) -> Result<(), TriggerError> {
        self.store_error()?;

        self.triggers
            .write()
            .unwrap()
            .retain(|t| t.config.service_id != service_id && t.config.workflow_id != workflow_id);
        Ok(())
    }

    pub fn remove_service(&self, service_id: ServiceID) -> Result<(), TriggerError> {
        self.store_error()?;

        self.triggers
            .write()
            .unwrap()
            .retain(|t| t.config.service_id != service_id);

        Ok(())
    }

    pub fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError> {
        self.store_error()?;

        self.triggers
            .read()
            .unwrap()
            .iter()
            .filter(|t| t.config.service_id == service_id)
            .map(|t| Ok(t.config.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use TriggerData;

    use super::*;

    #[test]
    fn mock_trigger_sends() {
        let actions = vec![
            TriggerAction {
                config: mock_evm_event_trigger_config("service1", "workflow1"),
                data: TriggerData::new_raw(b"foobar"),
            },
            TriggerAction {
                config: mock_evm_event_trigger_config("service2", "workflow2"),
                data: TriggerData::new_raw(b"zoomba"),
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
        let data = mock_evm_event_trigger_config("abcd", "abcd");
        triggers.add_trigger(data).unwrap();
    }

    #[test]
    fn mock_trigger_fails() {
        let triggers = MockTriggerManagerVec::failing();
        // ensure start fails
        triggers.start(AppContext::new()).unwrap_err();

        // ensure store fails
        let data = mock_evm_event_trigger_config("abcd", "abcd");
        triggers.add_trigger(data).unwrap_err();
    }
}
