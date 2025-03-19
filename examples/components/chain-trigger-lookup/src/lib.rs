use alloy_network::Ethereum;
use anyhow::Context;
use example_helpers::bindings::{
    compat::{TriggerData, TriggerDataCosmosContractEvent, TriggerDataEthContractEvent},
    world::{host, Guest, TriggerAction},
};
use example_helpers::{
    export_layer_trigger_world,
    trigger::{decode_trigger_event, encode_trigger_output, ChainQuerierExt},
};
use serde::{Deserialize, Serialize};
use wavs_wasi_utils::ethereum::new_eth_provider;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<Vec<u8>>, String> {
        wstd::runtime::block_on(async move {
            let (trigger_id, _) = decode_trigger_event(trigger_action.data.clone())?;

            let resp = match trigger_action.data {
                TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent {
                    chain_name,
                    contract_address,
                    ..
                }) => {
                    let chain_config = host::get_cosmos_chain_config(&chain_name).ok_or(
                        anyhow::anyhow!("cosmos chain config for {chain_name} not found"),
                    )?;

                    layer_climb::querier::QueryClient::new(chain_config.into(), None)
                        .await?
                        .trigger_data(contract_address.into(), trigger_id)
                        .await?
                }
                TriggerData::EthContractEvent(TriggerDataEthContractEvent {
                    chain_name,
                    contract_address,
                    ..
                }) => {
                    let chain_config = host::get_eth_chain_config(&chain_name).ok_or(
                        anyhow::anyhow!("eth chain config for {chain_name} not found"),
                    )?;

                    new_eth_provider::<Ethereum>(
                        chain_config
                            .http_endpoint
                            .context("http_endpoint is missing")?,
                    )
                    .trigger_data(contract_address.into(), trigger_id)
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

// The response from the contract query
#[derive(Deserialize, Serialize)]
struct TriggerDataResp {
    pub data: String,
}

export_layer_trigger_world!(Component);
