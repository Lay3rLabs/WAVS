// Helpers to work with "trigger id" flows - which our example components do
use alloy_sol_types::SolValue;
use anyhow::Result;
use example_submit::DataWithId;
use example_trigger::{NewTrigger, TriggerInfo};
use layer_wasi::{
    bindings::compat::{TriggerData, TriggerDataCosmosContractEvent, TriggerDataEthContractEvent},
    cosmos::CosmosQuerier,
    ethereum::EthereumQuerier,
};
use serde::{de::DeserializeOwned, Serialize};

pub fn decode_trigger_event(trigger_data: TriggerData) -> Result<(u64, Vec<u8>)> {
    match trigger_data {
        TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent { event, .. }) => {
            let event = cosmwasm_std::Event::from(event);
            let event = cosmos_contract_simple_example::event::NewMessageEvent::try_from(event)?;

            Ok((event.id.u64(), event.data))
        }
        TriggerData::EthContractEvent(TriggerDataEthContractEvent { log, .. }) => {
            let event: NewTrigger = layer_wasi::ethereum::decode_event_log_data(log)?;
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
    async fn trigger_data<T: DeserializeOwned>(
        &self,
        address: layer_climb_address::Address,
        trigger_id: u64,
    ) -> Result<T>;
}

impl ChainQuerierExt for CosmosQuerier {
    // on Cosmos, the contract *must* implement a handler for the QueryMsg::TriggerData variant
    async fn trigger_data<T: DeserializeOwned>(
        &self,
        address: layer_climb_address::Address,
        trigger_id: u64,
    ) -> Result<T> {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        enum QueryMsg {
            TriggerData { trigger_id: String },
        }

        self.contract_smart(
            &address,
            QueryMsg::TriggerData {
                trigger_id: trigger_id.to_string(),
            },
        )
        .await
    }
}

impl ChainQuerierExt for EthereumQuerier {
    // convenience helper for typical use-case of querying an ethereum event trigger
    async fn trigger_data<T: DeserializeOwned>(
        &self,
        _address: layer_climb_address::Address,
        _trigger_id: u64,
    ) -> Result<T> {
        todo!()
    }
}

mod example_trigger {
    use alloy_sol_macro::sol;
    pub use ISimpleTrigger::TriggerInfo;
    pub use SimpleTrigger::NewTrigger;

    sol!(
        #[allow(missing_docs)]
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
