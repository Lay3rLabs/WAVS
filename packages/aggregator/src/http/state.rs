use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use alloy::primitives::Address;
use anyhow::bail;
use utils::{
    layer_contract_client::SignedPayload,
    storage::db::{DBError, RedbStorage, Table, JSON},
};

use crate::config::Config;

// Hold a list of payloads for a given TriggerId
// TODO - optimizations:
// 1. maintain a count that doesn't need to load the whole thing
// 2. re-assess to see if we need to store the whole payload
// also, gotta move ServiceID to utils
pub type PayloadsByServiceId = HashMap<String, Vec<SignedPayload>>;

// Note: If service exists in db it's considered registered
const PAYLOADS_BY_SERVICE_ID: Table<&str, JSON<PayloadsByServiceId>> =
    Table::new("payloads_by_service_id");

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

    pub fn load_all_payloads(
        &self,
        service_manager: Address,
    ) -> anyhow::Result<PayloadsByServiceId> {
        match self
            .storage
            .get(PAYLOADS_BY_SERVICE_ID, &service_manager.to_string())?
        {
            Some(payloads) => Ok(payloads.value()),
            None => Err(anyhow::anyhow!(
                "Service manager at address {} is not registered",
                service_manager
            )),
        }
    }

    pub fn save_all_payloads(
        &self,
        service_manager: Address,
        payloads: PayloadsByServiceId,
    ) -> Result<(), DBError> {
        self.storage.set(
            PAYLOADS_BY_SERVICE_ID,
            &service_manager.to_string(),
            &payloads,
        )
    }

    pub fn register_service(
        &self,
        service_manager: Address,
        service_id: String,
    ) -> anyhow::Result<()> {
        let service_manager = service_manager.to_string();

        match self.storage.get(PAYLOADS_BY_SERVICE_ID, &service_manager)? {
            None => {
                let mut lookup = HashMap::new();
                lookup.insert(service_id, Vec::new());
                self.storage
                    .set(PAYLOADS_BY_SERVICE_ID, &service_manager, &lookup)?;
            }
            Some(table) => match table.value().entry(service_id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(Vec::new());
                }
                Entry::Occupied(_) => {
                    bail!(
                        "Service manager at {} is already registered for service {}",
                        service_manager,
                        service_id
                    );
                }
            },
        }

        Ok(())
    }
}
