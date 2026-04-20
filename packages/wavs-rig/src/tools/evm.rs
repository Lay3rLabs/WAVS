//! EvmQueryTool — read-only eth_call against an EVM contract.
//!
//! Uses raw JSON-RPC over wstd::http::Client to avoid alloy dependency issues
//! on wasm32-wasip2. This approach is consistent with how wavs-wasi-utils
//! implements EVM transport (WasiEvmClient).
//!
//! Only read-only eth_call is supported — no state mutations.

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wstd::http::{Body, Client, Request};

// ─── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum EvmQueryError {
    #[error("HTTP request failed: {0}")]
    HttpFailed(String),

    #[error("JSON-RPC error (code {code}): {message}")]
    RpcError { code: i64, message: String },

    #[error("Unexpected JSON-RPC response: {0}")]
    UnexpectedResponse(String),
}

// ─── Types ────────────────────────────────────────────────────────────────────

/// Arguments for EvmQueryTool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvmQueryArgs {
    /// HTTP(S) URL of the EVM JSON-RPC endpoint.
    pub rpc_url: String,
    /// 0x-prefixed hex-encoded contract address.
    pub to: String,
    /// 0x-prefixed hex-encoded ABI-encoded calldata.
    pub data: String,
}

// Internal JSON-RPC request shape.
#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: serde_json::Value,
    id: u64,
}

// Internal JSON-RPC response shape.
#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// ─── EvmQueryTool ─────────────────────────────────────────────────────────────

/// Execute a read-only eth_call against an EVM contract.
///
/// Sends a raw JSON-RPC eth_call request over wstd::http (wasi:http/outgoing-handler).
/// Only read operations are supported — no gas, no signer, no state mutations.
pub struct EvmQueryTool;

impl Tool for EvmQueryTool {
    const NAME: &'static str = "evm_query";

    type Error = EvmQueryError;
    type Args = EvmQueryArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a read-only eth_call against an EVM contract. \
                Provide the RPC URL, 0x-prefixed contract address, and 0x-prefixed ABI-encoded calldata. \
                Returns the 0x-prefixed hex-encoded return data."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(EvmQueryArgs))
                .unwrap_or(serde_json::Value::Null),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Build JSON-RPC eth_call payload.
        let rpc_request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "eth_call",
            params: serde_json::json!([
                {
                    "to": args.to,
                    "data": args.data,
                },
                "latest"
            ]),
            id: 1,
        };

        let body_bytes = serde_json::to_vec(&rpc_request)
            .map_err(|e| EvmQueryError::UnexpectedResponse(e.to_string()))?;

        let request = Request::post(&args.rpc_url)
            .header("content-type", "application/json")
            .body(Body::from(body_bytes))
            .map_err(|e| EvmQueryError::HttpFailed(e.to_string()))?;

        let mut response = Client::new()
            .send(request)
            .await
            .map_err(|e| EvmQueryError::HttpFailed(format!("{:#}", e)))?;

        let resp_bytes = response
            .body_mut()
            .contents()
            .await
            .map_err(|e| EvmQueryError::HttpFailed(format!("{:#}", e)))?;

        let rpc_resp: JsonRpcResponse = serde_json::from_slice(&resp_bytes)
            .map_err(|e| EvmQueryError::UnexpectedResponse(e.to_string()))?;

        // Handle JSON-RPC level errors first.
        if let Some(err) = rpc_resp.error {
            return Err(EvmQueryError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        // Extract the result hex string.
        match rpc_resp.result {
            Some(serde_json::Value::String(hex)) => Ok(hex),
            Some(other) => Ok(other.to_string()),
            None => Err(EvmQueryError::UnexpectedResponse(
                "JSON-RPC response has no result and no error".to_string(),
            )),
        }
    }
}
