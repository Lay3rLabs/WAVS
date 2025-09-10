mod world;

use serde::Deserialize;
use world::{
    host,
    wasi::http::{self, types::*},
    wavs::aggregator::aggregator::{AggregatorAction, Packet, SubmitAction},
    wavs::types::{
        chain::{AnyTxHash, EvmAddress},
        core::LogLevel,
    },
    Guest,
};

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

fn get_gas_price() -> Result<Option<u64>, String> {
    // Check if gas oracle is configured
    let api_key = match host::config_var("etherscan_api_key") {
        Some(key) => key,
        None => return Ok(None), // No API key configured, skip gas price
    };

    let strategy = host::config_var("gas_strategy").unwrap_or_else(|| "standard".to_string());

    // Create HTTP request following the pattern from coin_gecko example
    let req = OutgoingRequest::new(Headers::new());
    let _ = req.set_scheme(Some(&Scheme::Https));
    let _ = req.set_authority(Some("api.etherscan.io"));
    let _ = req.set_path_with_query(Some(&format!(
        "/api?module=gastracker&action=gasoracle&apikey={}",
        api_key
    )));

    // Send request
    let future_res = http::outgoing_handler::handle(req, None)
        .map_err(|err| format!("outgoing error code: {err}"))?;
    let future_res_pollable = future_res.subscribe();
    future_res_pollable.block();

    let res = future_res
        .get()
        .unwrap()
        .unwrap()
        .map_err(|err| format!("outgoing response error code: {err:?}"))?;

    // Check status code
    match res.status() {
        200 => {}
        429 => return Err("Rate limited by Etherscan".to_string()),
        status => return Err(format!("Unexpected status code from Etherscan: {status}")),
    }

    // Read response body
    let body = res.consume().unwrap();
    let stream = body.stream().unwrap();

    let mut buf = Vec::with_capacity(1024 * 2); // 2KB should be enough for gas oracle response
    while let Ok(mut bytes) = stream.blocking_read(1024 * 2) {
        if bytes.is_empty() {
            break;
        }
        buf.append(&mut bytes);
    }

    // Parse JSON response
    let response: EtherscanGasOracleResponse = serde_json::from_slice(&buf)
        .map_err(|e| format!("Failed to parse Etherscan response: {}", e))?;

    // Select gas price based on strategy
    let gas_price_str = match strategy.as_str() {
        "fast" => &response.result.fast_gas_price,
        "slow" | "safe" => &response.result.safe_gas_price,
        _ => &response.result.propose_gas_price, // default to standard
    };

    // Convert from Gwei string to Wei (u64)
    // Gas price from Etherscan is in Gwei, we need Wei
    let gas_price_gwei: f64 = gas_price_str
        .parse()
        .map_err(|e| format!("Failed to parse gas price: {}", e))?;

    // Convert Gwei to Wei (1 Gwei = 10^9 Wei)
    let gas_price_wei = (gas_price_gwei * 1_000_000_000.0) as u64;

    Ok(Some(gas_price_wei))
}

struct Component;

impl Guest for Component {
    fn process_packet(_pkt: Packet) -> Result<Vec<AggregatorAction>, String> {
        let chain = host::config_var("chain").ok_or("chain config variable is required")?;
        let service_handler_str = host::config_var("service_handler")
            .ok_or("service_handler config variable is required")?;

        let address: alloy_primitives::Address = service_handler_str
            .parse()
            .map_err(|e| format!("Failed to parse service handler address: {e}"))?;

        // Get gas price from Etherscan if configured
        let gas_price = get_gas_price().unwrap_or(None);

        if let Some(price) = gas_price {
            host::log(
                LogLevel::Info,
                &format!(
                    "Using gas price: {} Wei ({} Gwei)",
                    price,
                    price / 1_000_000_000
                ),
            );
        }

        let submit_action = SubmitAction {
            chain,
            contract_address: EvmAddress {
                raw_bytes: address.to_vec(),
            },
            gas_price,
        };

        Ok(vec![AggregatorAction::Submit(submit_action)])
    }

    fn handle_timer_callback(_packet: Packet) -> Result<Vec<AggregatorAction>, String> {
        Err("Not implemented yet".to_string())
    }

    fn handle_submit_callback(
        _packet: Packet,
        tx_result: Result<AnyTxHash, String>,
    ) -> Result<(), String> {
        match tx_result {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

export_aggregator_world!(Component);
