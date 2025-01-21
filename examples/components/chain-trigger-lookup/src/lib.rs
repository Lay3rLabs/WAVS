use example_helpers::trigger::{decode_trigger_event, encode_trigger_output, ChainQuerierExt};
use layer_wasi::{
    bindings::{
        compat::{TriggerData, TriggerDataCosmosContractEvent, TriggerDataEthContractEvent},
        world::{host, Guest, TriggerAction},
    },
    cosmos::new_cosmos_query_client,
    ethereum::EthereumQuerier,
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
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

                    new_cosmos_query_client(chain_config, reactor)
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

                    EthereumQuerier::new(chain_config, reactor)
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
    }
}

// The response from the contract query
#[derive(Deserialize, Serialize)]
struct TriggerDataResp {
    pub data: String,
}

export_layer_trigger_world!(Component);
