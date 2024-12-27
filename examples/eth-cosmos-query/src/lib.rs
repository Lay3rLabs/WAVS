#[allow(warnings)]
mod bindings;
mod rpc;

use anyhow::{anyhow, Result};
use layer_climb_address::*;
use layer_climb_config::*;
use layer_wasi::Reactor;
use serde::{Deserialize, Serialize};

struct Component;

use bindings::Guest;
impl Guest for Component {
    fn process_eth_trigger(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        process_eth_trigger(input)
    }
}

fn process_eth_trigger(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
    let req: CosmosQueryRequest = serde_json::from_slice(&input)
        .map_err(|e| anyhow!("Could not deserialize input request from JSON: {}", e))
        .unwrap();

    let resp = handle_request(req).map_err(|e| e.to_string())?;

    serde_json::to_vec(&resp).map_err(|e| e.to_string())
}

fn handle_request(req: CosmosQueryRequest) -> Result<CosmosQueryResponse> {
    let chain_config = ChainConfig {
        chain_id: "local-osmosis".parse().unwrap(),
        rpc_endpoint: Some("http://localhost:26657".to_string()),
        grpc_endpoint: None,
        grpc_web_endpoint: None,
        gas_price: 0.025,
        gas_denom: "uosmo".to_string(),
        address_kind: AddrKind::Cosmos {
            prefix: "osmo".to_string(),
        },
    };

    let resp = wstd::runtime::block_on(|reactor| async move {
        match req {
            CosmosQueryRequest::BlockHeight => get_block_height(chain_config, reactor)
                .await
                .map(CosmosQueryResponse::BlockHeight),
            CosmosQueryRequest::Balance { address } => get_balance(chain_config, reactor, address)
                .await
                .map(CosmosQueryResponse::Balance),
        }
    })?;

    Ok(resp)
}

async fn get_block_height(chain_config: ChainConfig, reactor: Reactor) -> Result<u64> {
    rpc::block(chain_config, reactor, None)
        .await
        .map(|resp| resp.block.header.height.into())
}

async fn get_balance(
    chain_config: ChainConfig,
    reactor: Reactor,
    address: Address,
) -> Result<String> {
    let req = layer_climb_proto::bank::QueryBalanceRequest {
        address: address.to_string(),
        denom: chain_config.gas_denom.clone(),
    };

    rpc::abci_protobuf_query(
        chain_config,
        reactor,
        "/cosmos.bank.v1beta1.Query/Balance",
        req,
        None,
    )
    .await
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
