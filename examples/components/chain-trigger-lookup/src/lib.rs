use alloy_network::Ethereum;
use anyhow::Context;
use example_helpers::bindings::world::{
    host,
    wavs::operator::{
        input::{
            TriggerAction, TriggerData, TriggerDataCosmosContractEvent, TriggerDataEvmContractEvent,
        },
        output::WasmResponse,
    },
    Guest,
};
use example_helpers::{
    export_layer_trigger_world,
    trigger::{decode_trigger_event, encode_trigger_output, ChainQuerierExt},
};
use wavs_wasi_utils::evm::new_evm_provider;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        wstd::runtime::block_on(async move {
            let (trigger_id, _) = decode_trigger_event(trigger_action.data.clone())?;

            let resp = match trigger_action.data {
                TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent {
                    chain,
                    contract_address,
                    ..
                }) => {
                    let chain_config = host::get_cosmos_chain_config(&chain)
                        .ok_or(anyhow::anyhow!("cosmos chain config for {chain} not found"))?;

                    layer_climb::querier::QueryClient::new(chain_config.into(), None)
                        .await?
                        .trigger_data(contract_address.into(), trigger_id)
                        .await?
                }
                TriggerData::EvmContractEvent(TriggerDataEvmContractEvent {
                    chain, log, ..
                }) => {
                    let chain_config = host::get_evm_chain_config(&chain)
                        .ok_or(anyhow::anyhow!("EVM chain config for {chain} not found"))?;

                    new_evm_provider::<Ethereum>(
                        chain_config
                            .http_endpoint
                            .context("http_endpoint is missing")?,
                    )
                    .trigger_data(log.address.into(), trigger_id)
                    .await?
                }
                _ => {
                    return Err(anyhow::anyhow!("expected cosmos contract event"));
                }
            };

            Ok(encode_trigger_output(trigger_id, resp))
        })
        .map_err(|e| e.to_string())
        .map(Some)
    }
}

export_layer_trigger_world!(Component);
