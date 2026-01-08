// Helpers to work with "trigger id" flows - which our example components do
use crate::bindings::world::wavs::{
    operator::{input as component_input, output as component_output},
    types::{
        events::{
            TriggerDataAtprotoEvent, TriggerDataCosmosContractEvent, TriggerDataEvmContractEvent,
            TriggerDataHypercoreAppend,
        },
        service::ServiceManager,
    },
};
use alloy_provider::RootProvider;
use alloy_sol_types::SolValue;
use anyhow::Result;
use cosmwasm_std::HexBinary;
use cw_wavs_mock_api::message_with_id::MessageWithId;
use example_submit::DataWithId;
use example_trigger::{NewTrigger, SimpleTrigger, TriggerInfo};
use wavs_wasi_utils::decode_event_log_data;

pub fn decode_trigger_event(trigger_data: component_input::TriggerData) -> Result<(u64, Vec<u8>)> {
    match trigger_data {
        component_input::TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent {
            event,
            ..
        }) => {
            let event = cosmwasm_std::Event::from(event);
            let event = cw_wavs_trigger_api::simple::PushMessageEvent::try_from(&event)?;

            Ok((event.trigger_id.u64(), event.data.to_vec()))
        }
        component_input::TriggerData::EvmContractEvent(TriggerDataEvmContractEvent {
            log, ..
        }) => {
            let event: NewTrigger = decode_event_log_data!(log.data)?;

            let trigger_info = TriggerInfo::abi_decode(&event.triggerData)?;
            Ok((trigger_info.triggerId, trigger_info.data.to_vec()))
        }
        component_input::TriggerData::Raw(bytes) => Ok((0, bytes)),
        component_input::TriggerData::AtprotoEvent(TriggerDataAtprotoEvent {
            record_data,
            sequence,
            ..
        }) => Ok((
            sequence.try_into().expect("Expected sequence to be u64"),
            record_data
                .expect("Record data was not provided")
                .as_bytes()
                .to_vec(),
        )),
        component_input::TriggerData::HypercoreAppend(TriggerDataHypercoreAppend {
            index,
            data,
            ..
        }) => Ok((index, data)),
        _ => Err(anyhow::anyhow!("Unsupported trigger data type")),
    }
}

pub fn encode_trigger_output(
    trigger_id: u64,
    output: impl AsRef<[u8]>,
    service_manager: ServiceManager,
) -> component_output::WasmResponse {
    match service_manager {
        ServiceManager::Evm(_) => evm_encode_trigger_output(trigger_id, output),
        ServiceManager::Cosmos(_) => cosmos_encode_trigger_output(trigger_id, output),
    }
}
// For EVM ServiceHandler contracts, encode output using DataWithId struct
fn evm_encode_trigger_output(
    trigger_id: u64,
    output: impl AsRef<[u8]>,
) -> component_output::WasmResponse {
    component_output::WasmResponse {
        payload: DataWithId {
            triggerId: trigger_id,
            data: output.as_ref().to_vec().into(),
        }
        .abi_encode(),
        ordering: None,
        event_id_salt: None,
    }
}

// For Cosmos ServiceHandler contracts, encode output using MessageWithId struct
fn cosmos_encode_trigger_output(
    trigger_id: u64,
    output: impl AsRef<[u8]>,
) -> component_output::WasmResponse {
    component_output::WasmResponse {
        payload: MessageWithId {
            trigger_id: cosmwasm_std::Uint64::from(trigger_id),
            message: cosmwasm_std::HexBinary::from(output.as_ref().to_vec()),
        }
        .to_bytes()
        .unwrap(),
        ordering: None,
        event_id_salt: None,
    }
}

// extension traits for Cosmos and EVM queriers to add Trigger support
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
        let resp: HexBinary = self
            .contract_smart(
                &address,
                &cw_wavs_trigger_api::simple::QueryMsg::TriggerMessage {
                    trigger_id: trigger_id.into(),
                },
            )
            .await?;

        Ok(resp.into())
    }
}

impl ChainQuerierExt for RootProvider {
    // convenience helper for typical use-case of querying an EVM event trigger
    async fn trigger_data(
        &self,
        address: layer_climb::prelude::Address,
        trigger_id: u64,
    ) -> Result<Vec<u8>> {
        let contract = SimpleTrigger::new(address.try_into()?, self);

        Ok(contract.getTrigger(trigger_id).call().await?.data.to_vec())
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
    use alloy_sol_types::sol;
    pub use ISimpleSubmit::DataWithId;

    sol!(
        #[allow(missing_docs)]
        #[sol(rpc)]
        ISimpleSubmit,
        "../../contracts/solidity/abi/ISimpleSubmit.sol/ISimpleSubmit.json"
    );
}
