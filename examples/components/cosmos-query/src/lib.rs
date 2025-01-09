#[allow(warnings)]
mod bindings;

use anyhow::{anyhow, Result};
use example_helpers::trigger::{decode_trigger_input, encode_trigger_output};
use layer_climb_address::Address;
use layer_climb_config::*;
use layer_wasi::cosmos::CosmosQuerier;
use serde::{Deserialize, Serialize};

struct Component;

use bindings::{Contract, Guest};

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let (trigger_id, input) = decode_trigger_input(input)?;

        let req: CosmosQueryRequest = serde_json::from_slice(&input)
            .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
            .unwrap();

        let resp = handle_request(req).map_err(|e| e.to_string())?;

        serde_json::to_vec(&resp)
            .map_err(|e| e.to_string())
            .map(|output| encode_trigger_output(trigger_id, output))
    }
}

fn handle_request(req: CosmosQueryRequest) -> Result<CosmosQueryResponse> {
    let resp = wstd::runtime::block_on(|reactor| async move {
        let chain_config = ChainConfig {
            chain_id: "local-osmosis".parse().unwrap(),
            rpc_endpoint: Some("http://127.0.0.1:26657".to_string()),
            grpc_endpoint: None,
            grpc_web_endpoint: None,
            gas_price: 0.025,
            gas_denom: "uosmo".to_string(),
            address_kind: AddrKind::Cosmos {
                prefix: "osmo".to_string(),
            },
        };

        let querier = CosmosQuerier::new(chain_config, reactor);

        match req {
            CosmosQueryRequest::BlockHeight => querier
                .block_height()
                .await
                .map(CosmosQueryResponse::BlockHeight),

            CosmosQueryRequest::Balance { address } => {
                querier.balance(&address).await.map(|coin| match coin {
                    Some(coin) => CosmosQueryResponse::Balance(coin.amount),
                    None => CosmosQueryResponse::Balance("0".to_string()),
                })
            }
        }
    })?;

    Ok(resp)
}

bindings::export!(Component with_types_in bindings);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight,
    Balance { address: Address },
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}
