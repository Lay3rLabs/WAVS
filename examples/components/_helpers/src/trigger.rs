// Helpers to work with "trigger id" flows - which our example components do
use alloy_sol_types::SolValue;
use anyhow::{anyhow, Result};
use example_submit::DataWithId;
use example_trigger::{NewTriggerId, TriggerInfo};
use layer_wasi::{
    collection::HashMapLike,
    cosmos::CosmosQuerier,
    ethereum::EthereumQuerier,
    wasi::Reactor,
    wit_bindings::{AnyChainConfig, AnyContract, AnyEvent, ChainConfigs},
};
use serde::{de::DeserializeOwned, Serialize};

#[macro_export]
macro_rules! query_trigger {
    (
        $data_type:ty,
        $input:expr,
        $reactor:expr
    ) => {{
        $crate::trigger::_query_trigger(
            $crate::layer_wasi::canonicalize_any_contract!(
                crate::bindings::lay3r::avs::layer_types::AnyContract,
                $input.contract.clone()
            ),
            $crate::layer_wasi::canonicalize_any_event!(
                crate::bindings::lay3r::avs::layer_types::AnyEvent,
                $input.event.clone()
            ),
            $crate::layer_wasi::canonicalize_chain_configs!(
                crate::bindings::lay3r::avs::layer_types::AnyChainConfig,
                $input.chain_configs.clone()
            ),
            $input.chain_name.clone(),
            $reactor,
        )
    }};
}

pub async fn _query_trigger<D: DeserializeOwned>(
    contract: layer_wasi::wit_bindings::AnyContract,
    event: layer_wasi::wit_bindings::AnyEvent,
    chain_configs: layer_wasi::wit_bindings::ChainConfigs,
    chain_name: String,
    reactor: Reactor,
) -> anyhow::Result<(u64, D)> {
    let trigger_id = decode_any_trigger_event_id(event)?;
    let address: layer_climb_address::Address = contract.into();

    let chain_config = chain_configs
        .iter()
        .find_map(|(k, v)| if *k == chain_name { Some(v) } else { None })
        .ok_or(anyhow!("chain {} not found", chain_name))?;

    let res = match chain_config.clone() {
        layer_wasi::wit_bindings::AnyChainConfig::Eth(chain_config) => {
            let querier =
                layer_wasi::ethereum::EthereumQuerier::new(chain_config.http_endpoint, reactor);
            querier.trigger_data(address, trigger_id).await
        }
        layer_wasi::wit_bindings::AnyChainConfig::Cosmos(chain_config) => {
            let querier = CosmosQuerier::new(chain_config.into(), reactor);
            querier.trigger_data(address, trigger_id).await
        }
    }?;

    Ok((trigger_id, res))
}
pub fn decode_any_trigger_event_id(event: layer_wasi::wit_bindings::AnyEvent) -> Result<u64> {
    match event {
        layer_wasi::wit_bindings::AnyEvent::Cosmos(event) => decode_cosmos_trigger_event_id(event),
        layer_wasi::wit_bindings::AnyEvent::Eth(event) => decode_eth_trigger_event_id(event),
    }
}

pub fn decode_eth_trigger_event_id(
    log_data: layer_wasi::wit_bindings::EthEventLogData,
) -> Result<u64> {
    let event: NewTriggerId = layer_wasi::ethereum::decode_event_log_data(log_data)?;

    Ok(event._0)
}

pub fn decode_cosmos_trigger_event_id(event: layer_wasi::wit_bindings::CosmosEvent) -> Result<u64> {
    let event = cosmwasm_std::Event::from(event);
    let event = cosmos_contract_simple_example::event::NewMessageEvent::try_from(event)?;

    Ok(event.id.u64())
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
    pub use SimpleTrigger::NewTriggerId;

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
