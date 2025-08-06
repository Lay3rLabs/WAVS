use anyhow::Result;
use example_contract_cosmwasm_service_manager::msg::ExecuteMsg;
use layer_climb::prelude::*;

pub struct SimpleCosmosManagerClient {
    pub signing_client: deadpool::managed::Object<SigningClientPoolManager>,
    pub contract_address: Address,
}

impl SimpleCosmosManagerClient {
    pub fn new(
        signing_client: deadpool::managed::Object<SigningClientPoolManager>,
        contract_address: Address,
    ) -> Self {
        Self {
            signing_client,
            contract_address,
        }
    }

    pub async fn set_operator_weight(
        &self,
        operator: alloy_primitives::Address,
        weight: u64,
    ) -> Result<()> {
        let msg = ExecuteMsg::SetSigningKey {
            operator: operator.into(),
            signing_key: operator.into(),
            weight: weight.into(),
        };
        self.signing_client
            .contract_execute(&self.contract_address, &msg, Vec::new(), None)
            .await?;
        Ok(())
    }
}
