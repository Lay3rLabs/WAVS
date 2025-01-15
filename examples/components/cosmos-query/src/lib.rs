use anyhow::anyhow;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use layer_climb_address::Address;
use layer_wasi::{
    bindings::worlds::any_contract_event::{Guest, Input},
    cosmos::CosmosQuerier,
    export_any_contract_event_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (trigger_id, req) = decode_trigger_event(input.event.into())?;

            let req: CosmosQueryRequest =
                serde_json::from_slice(&req).map_err(|e| anyhow!("{:?}", e))?;

            let resp = match req {
                CosmosQueryRequest::BlockHeight { chain_name } => {
                    let querier = CosmosQuerier::new_from_chain_name(
                        &chain_name,
                        &input.chain_configs.into(),
                        reactor,
                    )?;

                    querier
                        .block_height()
                        .await
                        .map(CosmosQueryResponse::BlockHeight)
                }

                CosmosQueryRequest::Balance {
                    chain_name,
                    address,
                } => {
                    let querier = CosmosQuerier::new_from_chain_name(
                        &chain_name,
                        &input.chain_configs.into(),
                        reactor,
                    )?;

                    querier.balance(&address).await.map(|coin| match coin {
                        Some(coin) => CosmosQueryResponse::Balance(coin.amount),
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

export_any_contract_event_world!(Component);
