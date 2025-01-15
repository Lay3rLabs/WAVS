use std::sync::{Mutex, RwLock};
use std::time::Duration;

use crate::apis::trigger::{
    Trigger, TriggerAction, TriggerConfig, TriggerData, TriggerError, TriggerManager,
};
use crate::apis::{IDError, ServiceID, WorkflowID};
use crate::test_utils::address::{
    rand_address_eth, rand_address_layer, rand_event_cosmos, rand_event_eth,
};

use layer_climb::prelude::Address;
use serde::Serialize;
use tokio::sync::mpsc;
use tracing::instrument;
use utils::context::AppContext;

pub fn mock_eth_event_trigger_config(
    service_id: impl TryInto<ServiceID, Error = IDError>,
    workflow_id: impl TryInto<WorkflowID, Error = IDError>,
) -> TriggerConfig {
    TriggerConfig::eth_contract_event(
        service_id,
        workflow_id,
        rand_address_eth(),
        "eth",
        rand_event_eth(),
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
        rand_address_layer(),
        "cosmos",
        rand_event_cosmos(),
    )
    .unwrap()
}

pub fn mock_eth_event_trigger() -> Trigger {
    Trigger::eth_contract_event(rand_address_eth(), "eth", rand_event_eth())
}

pub fn mock_cosmos_event_trigger() -> Trigger {
    Trigger::cosmos_contract_event(rand_address_layer(), "cosmos", rand_event_cosmos())
}

pub fn mock_cosmos_event_trigger_data(trigger_id: u64, data: impl AsRef<[u8]>) -> TriggerData {
    TriggerData::CosmosContractEvent {
        contract_address: rand_address_layer(),
        chain_name: "layer".to_string(),
        event: utils::example_cosmos_client::NewMessageEvent {
            id: trigger_id.into(),
            data: data.as_ref().to_vec(),
        }
        .into(),
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

impl TriggerManager for MockTriggerManagerVec {
    fn start(&self, ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
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

    fn add_trigger(&self, _trigger: TriggerConfig) -> Result<(), TriggerError> {
        self.store_error()?;

        // MockTriggerManagerVec doesn't allow adding new triggers, since they need their data too
        Ok(())
    }

    fn remove_trigger(
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

    fn remove_service(&self, service_id: ServiceID) -> Result<(), TriggerError> {
        self.store_error()?;

        self.triggers
            .write()
            .unwrap()
            .retain(|t| t.config.service_id != service_id);

        Ok(())
    }

    fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError> {
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

// This mock is currently only used in mock_e2e.rs
// it doesn't have the same coverage in unit tests here as MockTriggerManager
pub struct MockTriggerManagerChannel {
    sender: mpsc::Sender<TriggerAction>,
    receiver: Mutex<Option<mpsc::Receiver<TriggerAction>>>,
    trigger_datas: Mutex<Vec<TriggerConfig>>,
}

impl MockTriggerManagerChannel {
    #[allow(clippy::new_without_default)]
    #[instrument(level = "debug", fields(subsys = "TriggerManager"))]
    pub fn new(channel_bound: usize) -> Self {
        let (sender, receiver) = mpsc::channel(channel_bound);

        Self {
            receiver: Mutex::new(Some(receiver)),
            sender,
            trigger_datas: Mutex::new(Vec::new()),
        }
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    pub async fn send_trigger(
        &self,
        service_id: impl TryInto<ServiceID, Error = IDError> + std::fmt::Debug,
        workflow_id: impl TryInto<WorkflowID, Error = IDError> + std::fmt::Debug,
        contract_address: &Address,
        data: &(impl Serialize + std::fmt::Debug),
        chain_id: impl ToString + std::fmt::Debug,
    ) {
        self.sender
            .send(TriggerAction {
                config: match contract_address {
                    Address::Eth(_) => TriggerConfig::eth_contract_event(
                        service_id,
                        workflow_id,
                        contract_address.clone(),
                        chain_id,
                        rand_event_eth(),
                    )
                    .unwrap(),
                    Address::Cosmos { .. } => TriggerConfig::cosmos_contract_event(
                        service_id,
                        workflow_id,
                        contract_address.clone(),
                        chain_id,
                        hex::encode(rand_event_eth()),
                    )
                    .unwrap(),
                },
                data: TriggerData::new_raw(serde_json::to_string(data).unwrap().as_bytes()),
            })
            .await
            .unwrap();
    }
}

impl TriggerManager for MockTriggerManagerChannel {
    #[instrument(level = "debug", skip(self, _ctx), fields(subsys = "TriggerManager"))]
    fn start(&self, _ctx: AppContext) -> Result<mpsc::Receiver<TriggerAction>, TriggerError> {
        let receiver = self.receiver.lock().unwrap().take().unwrap();
        Ok(receiver)
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn add_trigger(&self, trigger: TriggerConfig) -> Result<(), TriggerError> {
        self.trigger_datas.lock().unwrap().push(trigger);
        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_trigger(
        &self,
        service_id: ServiceID,
        workflow_id: WorkflowID,
    ) -> Result<(), TriggerError> {
        self.trigger_datas
            .lock()
            .unwrap()
            .retain(|t| t.service_id != service_id && t.workflow_id != workflow_id);
        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn remove_service(&self, service_id: ServiceID) -> Result<(), TriggerError> {
        self.trigger_datas
            .lock()
            .unwrap()
            .retain(|t| t.service_id != service_id);
        Ok(())
    }

    #[instrument(level = "debug", skip(self), fields(subsys = "TriggerManager"))]
    fn list_triggers(&self, service_id: ServiceID) -> Result<Vec<TriggerConfig>, TriggerError> {
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

    use crate::apis::trigger::TriggerData;

    use super::*;

    #[test]
    fn mock_trigger_sends() {
        let actions = vec![
            TriggerAction {
                config: mock_eth_event_trigger_config("service1", "workflow1"),
                data: TriggerData::new_raw(b"foobar"),
            },
            TriggerAction {
                config: mock_eth_event_trigger_config("service2", "workflow2"),
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
        let data = mock_eth_event_trigger_config("abcd", "abcd");
        triggers.add_trigger(data).unwrap();
    }

    #[test]
    fn mock_trigger_fails() {
        let triggers = MockTriggerManagerVec::failing();
        // ensure start fails
        triggers.start(AppContext::new()).unwrap_err();

        // ensure store fails
        let data = mock_eth_event_trigger_config("abcd", "abcd");
        triggers.add_trigger(data).unwrap_err();
    }
}
