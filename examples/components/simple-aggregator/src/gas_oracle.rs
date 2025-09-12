use crate::world::{host, wavs::types::core::LogLevel};
use serde::{Deserialize, Serialize};
use wavs_wasi_utils::http::{fetch_json, http_request_post_json};
use wstd::runtime::block_on;

#[derive(Serialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<String>,
    id: u64,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: String,
}

pub fn get_gas_price() -> Result<Option<u64>, String> {
    let rpc_url = match std::env::var("WAVS_ENV_GAS_RPC_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => return Ok(None),
    };

    let strategy = host::config_var("gas_strategy").unwrap_or_else(|| "standard".to_string());

    host::log(
        LogLevel::Info,
        &format!("Fetching gas price from RPC: {rpc_url} with strategy: {strategy}"),
    );

    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "eth_gasPrice".to_string(),
        params: vec![],
        id: 1,
    };

    let response: RpcResponse = block_on(async {
        let http_request = http_request_post_json(&rpc_url, &request)
            .map_err(|e| format!("Failed to create RPC request: {e}"))?;
        
        fetch_json(http_request)
            .await
            .map_err(|e| format!("Failed to fetch gas price from RPC: {e}"))
    })?;

    let gas_price_hex = response.result.trim_start_matches("0x");
    let gas_price_wei = u64::from_str_radix(gas_price_hex, 16)
        .map_err(|e| format!("Invalid gas price hex from RPC: {e}"))?;

    let gas_price_gwei = gas_price_wei as f64 / 1_000_000_000.0;

    let adjusted_gas_price = match strategy.as_str() {
        "fast" => (gas_price_wei as f64 * 1.2) as u64,
        "slow" | "safe" => (gas_price_wei as f64 * 0.9) as u64,
        _ => gas_price_wei,
    };

    host::log(
        LogLevel::Info,
        &format!(
            "Successfully fetched gas price: {gas_price_gwei:.2} Gwei ({adjusted_gas_price} Wei)"
        ),
    );

    Ok(Some(adjusted_gas_price))
}