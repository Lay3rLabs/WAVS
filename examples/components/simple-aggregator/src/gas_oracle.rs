use crate::world::{host, wavs::types::core::LogLevel};
use alloy_network::Ethereum;
use alloy_primitives::utils::format_units;
use alloy_provider::Provider;
use anyhow::anyhow;
use wavs_wasi_utils::evm::new_evm_provider;
use wstd::runtime::block_on;

/// v * num / den with HALF-UP rounding and overflow checks
fn mul_div_round_u128(v: u128, num: u128, den: u128) -> Option<u128> {
    // (v * num + den/2) / den
    let prod = v.checked_mul(num)?;
    let adj = prod.checked_add(den / 2)?; // half-up
    Some(adj / den)
}

pub fn get_gas_price() -> anyhow::Result<Option<u64>> {
    let rpc_url = match std::env::var("WAVS_ENV_GAS_RPC_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => return Ok(None),
    };

    let strategy = host::config_var("gas_strategy").unwrap_or_else(|| "standard".to_string());

    host::log(
        LogLevel::Info,
        &format!("Fetching gas price from RPC: {rpc_url} with strategy: {strategy}"),
    );

    let provider = new_evm_provider::<Ethereum>(rpc_url);

    let gas_price_wei = block_on(async { provider.get_gas_price().await })?;

    let adjusted_gas_price: u128 = match strategy.to_lowercase().as_str() {
        "fast" => mul_div_round_u128(gas_price_wei, 12, 10)
            .ok_or_else(|| anyhow!("Overflow while computing fast gas price"))?,
        "slow" | "safe" => mul_div_round_u128(gas_price_wei, 9, 10)
            .ok_or_else(|| anyhow!("Overflow while computing slow/safe gas price"))?,
        _ => gas_price_wei,
    };

    host::log(
        LogLevel::Info,
        &format!(
            "Successfully fetched gas price: {0} Gwei (adjusted to {1} Gwei)",
            format_units(gas_price_wei, "gwei")?,
            format_units(adjusted_gas_price, "gwei")?
        ),
    );

    Ok(Some(adjusted_gas_price.try_into()?))
}
