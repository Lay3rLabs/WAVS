use crate::world::{host, wavs::types::core::LogLevel};
use serde::Deserialize;
use wavs_wasi_utils::http::{fetch_json, http_request_get};
use wstd::runtime::block_on;

#[derive(Deserialize)]
struct EtherscanGasOracleResponse {
    result: GasOracleResult,
}

#[derive(Deserialize)]
struct GasOracleResult {
    #[serde(rename = "SafeGasPrice")]
    safe_gas_price: String,
    #[serde(rename = "ProposeGasPrice")]
    propose_gas_price: String,
    #[serde(rename = "FastGasPrice")]
    fast_gas_price: String,
}

pub fn get_gas_price() -> Result<Option<u64>, String> {
    let api_key = match host::config_var("etherscan_api_key") {
        Some(key) if !key.is_empty() => key,
        _ => return Ok(None),
    };

    let strategy = host::config_var("gas_strategy").unwrap_or_else(|| "standard".to_string());

    host::log(
        LogLevel::Info,
        &format!("Fetching gas price from Etherscan with strategy: {strategy}"),
    );

    let url = format!("https://api.etherscan.io/api?module=gastracker&action=gasoracle&apikey={api_key}");
    
    let response: EtherscanGasOracleResponse = block_on(async {
        fetch_json(http_request_get(&url).map_err(|e| format!("Failed to create request: {e}"))?)
            .await
            .map_err(|e| format!("Failed to fetch gas price from Etherscan: {e}"))
    })?;

    let gas_price_str = match strategy.as_str() {
        "fast" => &response.result.fast_gas_price,
        "slow" | "safe" => &response.result.safe_gas_price,
        _ => &response.result.propose_gas_price,
    };

    let gas_price_gwei: f64 = gas_price_str
        .parse()
        .map_err(|e| format!("Invalid gas price from Etherscan: {e}"))?;

    if !(0.1..=10000.0).contains(&gas_price_gwei) {
        return Err(format!(
            "Unreasonable gas price from Etherscan: {gas_price_gwei} Gwei"
        ));
    }

    let gas_price_wei = (gas_price_gwei * 1_000_000_000.0) as u64;

    host::log(
        LogLevel::Info,
        &format!("Successfully fetched gas price: {gas_price_gwei} Gwei ({gas_price_wei} Wei)"),
    );

    Ok(Some(gas_price_wei))
}