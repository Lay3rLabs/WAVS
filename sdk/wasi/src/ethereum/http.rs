use alloy_json_rpc::{Request as JsonRpcRequest, Response as JsonRpcResponse};
use alloy_primitives::{hex, Address};
use alloy_sol_types::SolCall;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use wasi::http::types::Method;

use crate::wasi::{Request, WasiPollable};

#[derive(Serialize, Debug, Clone, Deserialize)]
struct EthCallParams {
    to: String,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "gasPrice")]
    gas_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

pub async fn eth_call_http_raw<Fun>(
    reactor: &wstd::runtime::Reactor,
    http_url: &str,
    contract_address: Address,
    args: <<Fun as SolCall>::Parameters<'_> as alloy_sol_types::SolType>::RustType,
) -> Result<Fun::Return>
where
    Fun: alloy_sol_types::SolCall,
{
    let call_data = Fun::new(args).abi_encode();

    // 2) Build the JSON-RPC request
    let call_params = EthCallParams {
        to: format!("{:#x}", contract_address),
        data: format!("0x{}", hex::encode(call_data)),
        from: None,
        gas: None,
        gas_price: None,
        value: None,
    };

    let rpc_req = JsonRpcRequest::new("eth_call", 1.into(), call_params);

    let mut req = Request::new(Method::Post, http_url).map_err(|e| anyhow::anyhow!("{:?}", e))?;

    req.body = serde_json::to_vec(&rpc_req)?;
    req.headers
        .push(("content-type".to_string(), "application/json".to_string()));

    let res = reactor
        .send(req)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    match res.status {
        200 => {
            let response: JsonRpcResponse =
                serde_json::from_slice(&res.body).map_err(|err| anyhow::anyhow!(err))?;

            match response.payload {
                alloy_json_rpc::ResponsePayload::Success(response) => {
                    let data = hex::decode(response.get())?;
                    let res = Fun::abi_decode_returns(&data, false)?;

                    Ok(res)
                }
                alloy_json_rpc::ResponsePayload::Failure(error_payload) => {
                    Err(anyhow::anyhow!("RPC error: {:?}", error_payload))
                }
            }
        }
        status => Err(anyhow::anyhow!("unexpected status code: {status}")),
    }
}
