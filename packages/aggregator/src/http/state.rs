use std::sync::Arc;

use alloy::primitives::Address;
use utils::{
    avs_client::SignedPayload,
    storage::db::{DBError, RedbStorage, Table, JSON},
};

use crate::config::Config;

pub type PayloadsByContractAddress = Vec<SignedPayload>;

// Note: If service exists in db it's considered registered
const PAYLOADS_BY_CONTRACT_ADDRESS: Table<&str, JSON<PayloadsByContractAddress>> =
    Table::new("payloads_by_contract_address");

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
    ) -> anyhow::Result<PayloadsByContractAddress> {
        match self
            .storage
            .get(PAYLOADS_BY_CONTRACT_ADDRESS, &service_manager.to_string())?
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
        payloads: PayloadsByContractAddress,
    ) -> Result<(), DBError> {
        self.storage.set(
            PAYLOADS_BY_CONTRACT_ADDRESS,
            &service_manager.to_string(),
            &payloads,
        )
    }

    pub fn register_service(&self, service_manager: Address) -> anyhow::Result<()> {
        let service_manager = service_manager.to_string();

        if self
            .storage
            .get(PAYLOADS_BY_CONTRACT_ADDRESS, &service_manager)?
            .is_none()
        {
            self.storage
                .set(PAYLOADS_BY_CONTRACT_ADDRESS, &service_manager, &Vec::new())?;
        }

        Ok(())
    }
}
