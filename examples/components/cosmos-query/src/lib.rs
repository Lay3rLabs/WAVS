use anyhow::anyhow;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use layer_climb_address::Address;
use layer_wasi::{
    bindings::world::{host, Guest, TriggerAction},
    cosmos::new_cosmos_query_client,
    export_layer_trigger_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (trigger_id, req) = decode_trigger_event(trigger_action.data)?;

            let req: CosmosQueryRequest =
                serde_json::from_slice(&req).map_err(|e| anyhow!("{:?}", e))?;

            let resp = match req {
                CosmosQueryRequest::BlockHeight { chain_name } => {
                    let chain_config = host::get_cosmos_chain_config(&chain_name)
                        .ok_or(anyhow!("chain config for {chain_name} not found"))?;
                    let querier = new_cosmos_query_client(chain_config, reactor).await?;

                    querier
                        .block_height()
                        .await
                        .map(CosmosQueryResponse::BlockHeight)
                }

                CosmosQueryRequest::Balance {
                    chain_name,
                    address,
                } => {
                    let chain_config = host::get_cosmos_chain_config(&chain_name)
                        .ok_or(anyhow!("chain config for {chain_name} not found"))?;
                    let querier = new_cosmos_query_client(chain_config, reactor).await?;

                    querier
                        .balance(address, None)
                        .await
                        .map(|amount| match amount {
                            Some(amount) => CosmosQueryResponse::Balance(amount.to_string()),
                            None => CosmosQueryResponse::Balance("0".to_string()),
                        })
                }
            }?;

            serde_json::to_vec(&resp)
                .map_err(|e| anyhow!("{:?}", e))
                .map(|output| encode_trigger_output(trigger_id, output))
        })
        .map_err(|e| e.to_string())
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight {
        chain_name: String,
    },
    Balance {
        chain_name: String,
        address: Address,
    },
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}

export_layer_trigger_world!(Component);
