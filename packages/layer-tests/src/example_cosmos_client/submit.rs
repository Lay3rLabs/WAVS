use crate::example_evm_client::example_submit::ISimpleSubmit::SignedData;
use alloy_sol_types::SolValue;
use anyhow::Result;
use cosmwasm_std::{Binary, Uint64};
use example_contract_cosmwasm_service_handler::msg::{InstantiateMsg, QueryMsg};
use layer_climb::prelude::*;

pub struct SimpleCosmosSubmitClient {
    pub signing_client: deadpool::managed::Object<SigningClientPoolManager>,
    pub contract_address: Address,
}

impl SimpleCosmosSubmitClient {
    pub fn new(
        signing_client: deadpool::managed::Object<SigningClientPoolManager>,
        contract_address: Address,
    ) -> Self {
        Self {
            signing_client,
            contract_address,
        }
    }

    pub async fn new_code_id(
        signing_client: deadpool::managed::Object<SigningClientPoolManager>,
        code_id: u64,
        service_manager: &layer_climb::prelude::Address,
        label: &str,
    ) -> Result<Self> {
        let msg = InstantiateMsg {
            service_manager: service_manager.to_string(),
        };
        let (addr, _) = signing_client
            .contract_instantiate(None, code_id, label, &msg, Vec::new(), None)
            .await?;

        Ok(Self::new(signing_client, addr))
    }

    pub async fn trigger_validated(&self, trigger_id: u64) -> Result<bool> {
        let msg = QueryMsg::TriggerValidated {
            trigger_id: Uint64::from(trigger_id),
        };
        self.signing_client
            .querier
            .contract_smart(&self.contract_address, &msg)
            .await
    }

    pub async fn signed_data(&self, trigger_id: u64) -> Result<SignedData> {
        let msg = QueryMsg::SignedData {
            trigger_id: Uint64::from(trigger_id),
        };

        let signed_data: Binary = self
            .signing_client
            .querier
            .contract_smart(&self.contract_address, &msg)
            .await?;

        SignedData::abi_decode(signed_data.as_slice())
            .map_err(|e| anyhow::anyhow!("Failed to decode SignedData: {e:?}"))
    }
}
