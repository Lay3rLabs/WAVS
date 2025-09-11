use crate::world::{
    host,
    wasi::http::{self, types::*},
    wavs::types::core::LogLevel,
};
use serde::Deserialize;

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
        _ => return Ok(None), // no API key configured, skip gas price fetching
    };

    let strategy = host::config_var("gas_strategy").unwrap_or_else(|| "standard".to_string());

    // when API key is configured, gas price fetching is REQUIRED to succeed
    host::log(
        LogLevel::Info,
        &format!("Fetching gas price from Etherscan with strategy: {strategy}"),
    );

    let req = OutgoingRequest::new(Headers::new());
    req.set_scheme(Some(&Scheme::Https))
        .map_err(|_| "Failed to set HTTPS scheme for Etherscan request")?;
    req.set_authority(Some("api.etherscan.io"))
        .map_err(|_| "Failed to set authority for Etherscan request")?;
    req.set_path_with_query(Some(&format!(
        "/api?module=gastracker&action=gasoracle&apikey={api_key}"
    )))
    .map_err(|_| "Failed to set path for Etherscan request")?;

    let future_res = http::outgoing_handler::handle(req, None)
        .map_err(|err| format!("Failed to send Etherscan request: {err}"))?;
    let future_res_pollable = future_res.subscribe();
    future_res_pollable.block();

    let res = future_res
        .get()
        .unwrap()
        .unwrap()
        .map_err(|err| format!("Etherscan request failed: {err:?}"))?;

    match res.status() {
        200 => {}
        429 => return Err("Etherscan API rate limit exceeded - cannot fetch gas price".to_string()),
        401 | 403 => return Err("Invalid or unauthorized Etherscan API key".to_string()),
        status => return Err(format!("Etherscan API returned error status: {status}")),
    }

    let body = res.consume().unwrap();
    let stream = body.stream().unwrap();

    let mut buf = Vec::with_capacity(1024 * 2); // 2KB should be enough for gas oracle response
    while let Ok(mut bytes) = stream.blocking_read(1024 * 2) {
        if bytes.is_empty() {
            break;
        }
        buf.append(&mut bytes);
    }

    let response: EtherscanGasOracleResponse = serde_json::from_slice(&buf)
        .map_err(|e| format!("Failed to parse Etherscan JSON response: {e}"))?;

    let gas_price_str = match strategy.as_str() {
        "fast" => &response.result.fast_gas_price,
        "slow" | "safe" => &response.result.safe_gas_price,
        _ => &response.result.propose_gas_price, // default to standard
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
