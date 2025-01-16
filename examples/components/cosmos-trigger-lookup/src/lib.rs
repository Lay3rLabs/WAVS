use example_helpers::trigger::{decode_trigger_event, ChainQuerierExt};
use layer_wasi::{
    bindings::{
        compat::{TriggerData, TriggerDataCosmosContractEvent},
        world::{host, Guest, TriggerAction},
    },
    cosmos::CosmosQuerier,
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (chain_name, contract_address) = match &trigger_action.data {
                TriggerData::CosmosContractEvent(TriggerDataCosmosContractEvent {
                    chain_name,
                    contract_address,
                    ..
                }) => (chain_name.clone(), contract_address.clone()),
                _ => {
                    return Err(anyhow::anyhow!("expected cosmos contract event"));
                }
            };

            let (trigger_id, _) = decode_trigger_event(trigger_action.data)?;

            let chain_config = host::get_cosmos_chain_config(&chain_name)
                .ok_or(anyhow::anyhow!("chain config for {chain_name} not found"))?;
            let querier = CosmosQuerier::new(chain_config, reactor);

            let resp: TriggerDataResp = querier
                .trigger_data(contract_address.into(), trigger_id)
                .await?;

            let resp = serde_json::to_vec(&resp)?;

            serde_json::to_vec(&Response {
                trigger_id: trigger_id.to_string(),
                result: resp,
            })
            .map_err(|e| anyhow::anyhow!("{:?}", e))
        })
        .map_err(|e| e.to_string())
    }
}

// The response we send back from the component, serialized to a Vec<u8>
#[derive(Serialize)]
struct Response {
    pub result: Vec<u8>,
    pub trigger_id: String,
}

// The response from the contract query
#[derive(Deserialize, Serialize)]
struct TriggerDataResp {
    pub data: String,
}

export_layer_trigger_world!(Component);
