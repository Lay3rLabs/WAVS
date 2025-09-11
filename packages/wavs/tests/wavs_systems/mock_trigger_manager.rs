#![allow(clippy::result_large_err)]
use std::sync::RwLock;
use std::time::Duration;

use utils::test_utils::address::{
    rand_address_cosmos, rand_address_evm, rand_event_cosmos, rand_event_evm,
};
use wavs::subsystems::trigger::error::TriggerError;

use alloy_primitives::LogData;
use serde::Serialize;
use tokio::sync::mpsc;
use utils::context::AppContext;
use wavs_types::{
    ChainKey, ChainKeyError, ServiceId, Trigger, TriggerAction, TriggerConfig, TriggerData,
    WorkflowId, WorkflowIdError,
};

pub fn mock_real_trigger_action(
    service_id: ServiceId,
    workflow_id: impl TryInto<WorkflowId, Error = WorkflowIdError> + std::fmt::Debug,
    contract_address: &layer_climb::prelude::Address,
    data: &(impl Serialize + std::fmt::Debug),
    chain: impl TryInto<ChainKey, Error = ChainKeyError> + std::fmt::Debug + Clone,
) -> TriggerAction {
    let data = serde_json::to_vec(data).unwrap();
    match contract_address {
        layer_climb::prelude::Address::Evm(_) => {
            let event = rand_event_evm();
            TriggerAction {
                config: TriggerConfig::evm_contract_event(
                    service_id,
                    workflow_id,
                    contract_address.clone().try_into().unwrap(),
                    chain.clone(),
                    event,
                ),
                data: TriggerData::EvmContractEvent {
                    contract_address: contract_address.clone().try_into().unwrap(),
                    chain: chain.try_into().unwrap(),
                    // FIXME: this should be a proper EVM event, this is just a placeholder
                    log_data: LogData::new(vec![event.into_inner().into()], data.into()).unwrap(),
                    tx_hash: [0; 32].into(),
                    block_number: 1,
                    log_index: 0,
                    block_hash: [0; 32].into(),
                    block_timestamp: 0,
                    tx_index: 0,
                },
            }
        }
        layer_climb::prelude::Address::Cosmos { .. } => {
            let event = rand_event_cosmos();

            TriggerAction {
                config: TriggerConfig::cosmos_contract_event(
                    service_id,
                    workflow_id,
                    contract_address.clone(),
                    chain.clone(),
                    event.clone(),
                ),
                data: TriggerData::CosmosContractEvent {
                    contract_address: contract_address.clone(),
                    chain: chain.try_into().unwrap(),
                    event: cosmwasm_std::Event::new("new-message").add_attributes(vec![
                        ("id", "1".to_string()),
                        ("data", const_hex::encode(data)),
                    ]),
                    block_height: 1,
                    event_index: 0,
                },
            }
        }
    }
}

pub fn mock_evm_event_trigger_config(
    service_id: ServiceId,
    workflow_id: impl TryInto<WorkflowId, Error = WorkflowIdError>,
) -> TriggerConfig {
    TriggerConfig::evm_contract_event(
        service_id,
        workflow_id,
        rand_address_evm(),
        "evm:anvil",
        rand_event_evm(),
    )
}

pub fn mock_cosmos_event_trigger_config(
    service_id: ServiceId,
    workflow_id: impl TryInto<WorkflowId, Error = WorkflowIdError>,
) -> TriggerConfig {
    TriggerConfig::cosmos_contract_event(
        service_id,
        workflow_id,
        rand_address_cosmos(),
        "cosmos:wasmd",
        rand_event_cosmos(),
    )
}

pub fn mock_evm_event_trigger() -> Trigger {
    Trigger::evm_contract_event(rand_address_evm(), "evm:anvil", rand_event_evm())
}

pub fn mock_cosmos_event_trigger() -> Trigger {
    Trigger::cosmos_contract_event(rand_address_cosmos(), "cosmos:wasmd", rand_event_cosmos())
}

pub fn mock_cosmos_event_trigger_data(trigger_id: u64, data: impl AsRef<[u8]>) -> TriggerData {
    TriggerData::CosmosContractEvent {
        contract_address: rand_address_cosmos(),
        chain: "cosmos:layer".parse().unwrap(),
        // matches example_cosmos_client::NewMessageEvent
        event: cosmwasm_std::Event::new("new-message")
            .add_attribute("id", trigger_id.to_string())
            .add_attribute("data", const_hex::encode(data.as_ref())),
        block_height: 0,
        event_index: 0,
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
            true => Err(TriggerError::NoSuchService(ServiceId::hash("cant-start"))),
            false => Ok(()),
        }
    }

    fn store_error(&self) -> Result<(), TriggerError> {
        match self.error_on_store {
            true => Err(TriggerError::NoSuchService(ServiceId::hash("cant-store"))),
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
        service_id: ServiceId,
        workflow_id: WorkflowId,
    ) -> Result<(), TriggerError> {
        self.store_error()?;

        self.triggers
            .write()
            .unwrap()
            .retain(|t| t.config.service_id != service_id && t.config.workflow_id != workflow_id);
        Ok(())
    }

    pub fn remove_service(&self, service_id: ServiceId) -> Result<(), TriggerError> {
        self.store_error()?;

        self.triggers
            .write()
            .unwrap()
            .retain(|t| t.config.service_id != service_id);

        Ok(())
    }

    pub fn list_triggers(&self, service_id: ServiceId) -> Result<Vec<TriggerConfig>, TriggerError> {
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
                config: mock_evm_event_trigger_config(ServiceId::hash("service1"), "workflow1"),
                data: TriggerData::new_raw(b"foobar"),
            },
            TriggerAction {
                config: mock_evm_event_trigger_config(ServiceId::hash("service2"), "workflow2"),
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
        let data = mock_evm_event_trigger_config(ServiceId::hash("abcd"), "abcd");
        triggers.add_trigger(data).unwrap();
    }

    #[test]
    fn mock_trigger_fails() {
        let triggers = MockTriggerManagerVec::failing();
        // ensure start fails
        triggers.start(AppContext::new()).unwrap_err();

        // ensure store fails
        let data = mock_evm_event_trigger_config(ServiceId::hash("abcd"), "abcd");
        triggers.add_trigger(data).unwrap_err();
    }
}
