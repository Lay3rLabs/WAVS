#[allow(warnings)]
mod bindings;

use anyhow::anyhow;
use example_helpers::{query_trigger, trigger::encode_trigger_output};
use layer_climb_address::Address;
use layer_wasi::{canonicalize_chain_configs, cosmos::CosmosQuerier};
use serde::{Deserialize, Serialize};

struct Component;

use bindings::{Guest, Input};

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let (trigger_id, req) =
                query_trigger!(CosmosQueryRequest, &input, reactor.clone()).await?;
            let chain_configs = canonicalize_chain_configs!(
                crate::bindings::lay3r::avs::layer_types::AnyChainConfig,
                input.chain_configs
            );

            let resp = match req {
                CosmosQueryRequest::BlockHeight { chain_name } => {
                    let querier =
                        CosmosQuerier::new_from_chain_name(&chain_name, &chain_configs, reactor)?;

                    querier
                        .block_height()
                        .await
                        .map(CosmosQueryResponse::BlockHeight)
                }

                CosmosQueryRequest::Balance {
                    chain_name,
                    address,
                } => {
                    let querier =
                        CosmosQuerier::new_from_chain_name(&chain_name, &chain_configs, reactor)?;

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

bindings::export!(Component with_types_in bindings);

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
