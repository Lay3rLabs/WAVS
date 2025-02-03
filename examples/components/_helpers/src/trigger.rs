// Helpers to work with "trigger id" flows - which our example components do
use crate::bindings::compat::{
    TriggerData, TriggerDataCosmosContractEvent, TriggerDataEthContractEvent,
};
use alloy_sol_types::SolValue;
use anyhow::Result;
use example_submit::DataWithId;
use example_trigger::{NewTrigger, SimpleTrigger, TriggerInfo};
use serde::{Deserialize, Serialize};
use wavs_wasi_chain::{decode_event_log_data, ethereum::WasiProvider};

pub fn decode_trigger_event(trigger_data: TriggerData) -> Result<(u64, Vec<u8>)> {
    match trigger_data {
        TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent { event, .. }) => {
            let event = cosmwasm_std::Event::from(event);
            let event = cosmos_contract_simple_example::event::NewMessageEvent::try_from(event)?;

            Ok((event.id.u64(), event.data))
        }
        TriggerData::EthContractEvent(TriggerDataEthContractEvent { log, .. }) => {
            let event: NewTrigger = decode_event_log_data!(log)?;

            let trigger_info = TriggerInfo::abi_decode(&event._0, false)?;
            Ok((trigger_info.triggerId, trigger_info.data.to_vec()))
        }
        _ => Err(anyhow::anyhow!("Unsupported trigger data type")),
    }
}

pub fn encode_trigger_output(trigger_id: u64, output: impl AsRef<[u8]>) -> Vec<u8> {
    DataWithId {
        triggerId: trigger_id,
        data: output.as_ref().to_vec().into(),
    }
    .abi_encode()
}

// extension traits for Cosmos and Ethereum queriers to add Trigger support
#[allow(async_fn_in_trait)]
pub trait ChainQuerierExt {
    async fn trigger_data(
        &self,
        address: layer_climb::prelude::Address,
        trigger_id: u64,
    ) -> Result<Vec<u8>>;
}

//new_cosmos_query_client
impl ChainQuerierExt for layer_climb::prelude::QueryClient {
    // on Cosmos, the contract *must* implement a handler for the QueryMsg::TriggerData variant
    async fn trigger_data(
        &self,
        address: layer_climb::prelude::Address,
        trigger_id: u64,
    ) -> Result<Vec<u8>> {
        #[derive(Serialize, Debug)]
        #[serde(rename_all = "snake_case")]
        enum QueryMsg {
            TriggerData { trigger_id: String },
        }

        // The response from the contract query
        #[derive(Deserialize, Debug)]
        struct TriggerDataResp {
            pub data: Vec<u8>,
        }

        let resp: TriggerDataResp = self
            .contract_smart(
                &address,
                &QueryMsg::TriggerData {
                    trigger_id: trigger_id.to_string(),
                },
            )
            .await?;

        Ok(resp.data)
    }
}

impl ChainQuerierExt for WasiProvider {
    // convenience helper for typical use-case of querying an ethereum event trigger
    async fn trigger_data(
        &self,
        address: layer_climb::prelude::Address,
        trigger_id: u64,
    ) -> Result<Vec<u8>> {
        let contract = SimpleTrigger::new(address.try_into()?, self);

        Ok(contract
            .getTrigger(trigger_id)
            .call()
            .await?
            ._0
            .data
            .to_vec())
    }
}

mod example_trigger {
    use alloy_sol_macro::sol;
    pub use ISimpleTrigger::TriggerInfo;
    pub use SimpleTrigger::NewTrigger;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        SimpleTrigger,
        "../../contracts/solidity/abi/SimpleTrigger.sol/SimpleTrigger.json"
    );
}

mod example_submit {
    use alloy_sol_macro::sol;
    pub use ISimpleSubmit::DataWithId;

    sol!(
        #[allow(missing_docs)]
        ISimpleSubmit,
        "../../contracts/solidity/abi/ISimpleSubmit.sol/ISimpleSubmit.json"
    );
}
