use std::{collections::HashMap, sync::Arc};

use alloy::primitives::Address;
use anyhow::bail;
use utils::{
    layer_contract_client::{SignedPayload, TriggerId},
    storage::db::{DBError, RedbStorage, Table, JSON},
};

use crate::config::Config;

// Hold a list of payloads for a given TriggerId
// TODO - optimizations:
// 1. maintain a count that doesn't need to load the whole thing
// 2. re-assess to see if we need to store the whole payload
pub type TriggerPayloads = HashMap<TriggerId, Vec<SignedPayload>>;

// Note: If service exists in db it's considered registered
const TRIGGER_PAYLOADS: Table<&str, JSON<TriggerPayloads>> = Table::new("trigger-payloads");

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub storage: Arc<RedbStorage>,
}

// Note: task queue size is bounded by quorum and cleared on execution
impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let storage = Arc::new(RedbStorage::new(config.data.join("db"))?);
        Ok(Self { config, storage })
    }

    pub fn load_trigger_payloads(
        &self,
        service_manager: Address,
    ) -> anyhow::Result<TriggerPayloads> {
        match self
            .storage
            .get(TRIGGER_PAYLOADS, &service_manager.to_string())?
        {
            Some(payloads) => Ok(payloads.value()),
            None => Err(anyhow::anyhow!(
                "Service manager {} is not registered for triggers",
                service_manager
            )),
        }
    }

    pub fn save_trigger_payloads(
        &self,
        service_manager: Address,
        tasks: TriggerPayloads,
    ) -> Result<(), DBError> {
        self.storage
            .set(TRIGGER_PAYLOADS, &service_manager.to_string(), &tasks)
    }

    pub fn register_trigger_service(&self, service_manager: Address) -> anyhow::Result<()> {
        let service_manager = service_manager.to_string();
        match self.storage.get(TRIGGER_PAYLOADS, &service_manager)? {
            Some(_) => {
                bail!(
                    "Service manager at {} is already registered for triggers",
                    service_manager
                );
            }
            None => {
                self.storage
                    .set(TRIGGER_PAYLOADS, &service_manager, &HashMap::default())?;
            }
        }
        Ok(())
    }
}
