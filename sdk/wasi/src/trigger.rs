// convenience helper for the use-case of querying trigger data from a contract
// where the event data is the trigger-id (a string, but still emitted as bytes on-the-wire, like any other event data)
use anyhow::Result;
use layer_climb_address::*;
use serde::{de::DeserializeOwned, Serialize};

use crate::{cosmos::CosmosQuerier, ethereum::EthereumQuerier};

impl CosmosQuerier {
    // on Cosmos, the contract *must* implement a handler for the QueryMsg::TriggerData variant
    pub async fn event_trigger<T: DeserializeOwned>(
        &self,
        address: Address,
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

impl EthereumQuerier {
    // convenience helper for typical use-case of querying an ethereum event trigger
    pub async fn event_trigger<T: DeserializeOwned>(
        &self,
        _address: Address,
        _event_data: Vec<u8>,
    ) -> Result<(String, T)> {
        todo!()
    }
}
