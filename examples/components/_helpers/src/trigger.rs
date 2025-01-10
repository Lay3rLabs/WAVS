// Helpers to work with "trigger id" flows - which our example components do
use alloy_sol_types::SolValue;
use anyhow::Result;
use example_submit::DataWithId;
use example_trigger::TriggerInfo;
use layer_wasi::{cosmos::CosmosQuerier, ethereum::EthereumQuerier};
use serde::{de::DeserializeOwned, Serialize};

pub fn decode_trigger_input(input: Vec<u8>) -> std::result::Result<(u64, Vec<u8>), String> {
    TriggerInfo::abi_decode(&input, false)
        .map_err(|e| e.to_string())
        .map(|t| (t.triggerId, t.data.to_vec()))
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
    async fn event_trigger<T: DeserializeOwned>(
        &self,
        address: layer_climb_address::Address,
        event_data: Vec<u8>,
    ) -> Result<(String, T)>;
}

impl ChainQuerierExt for CosmosQuerier {
    // on Cosmos, the contract *must* implement a handler for the QueryMsg::TriggerData variant
    async fn event_trigger<T: DeserializeOwned>(
        &self,
        address: layer_climb_address::Address,
        event_data: Vec<u8>,
    ) -> Result<(String, T)> {
        let trigger_id = String::from_utf8(event_data)?;

        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        enum QueryMsg {
            TriggerData { trigger_id: String },
        }

        self.contract_smart(
            &address,
            QueryMsg::TriggerData {
                trigger_id: trigger_id.clone(),
            },
        )
        .await
        .map(move |data| (trigger_id, data))
    }
}

impl ChainQuerierExt for EthereumQuerier {
    // convenience helper for typical use-case of querying an ethereum event trigger
    async fn event_trigger<T: DeserializeOwned>(
        &self,
        _address: layer_climb_address::Address,
        _event_data: Vec<u8>,
    ) -> Result<(String, T)> {
        todo!()
    }
}

mod example_trigger {
    use alloy_sol_macro::sol;
    pub use ISimpleTrigger::TriggerInfo;

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
