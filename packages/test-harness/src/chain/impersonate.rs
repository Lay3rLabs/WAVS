//! Account impersonation and funding helpers for forked / Anvil chains.
//!
//! All helpers use Anvil's cheat-code RPC methods; they will fail on chains that
//! don't expose `anvil_*` namespaced methods.

use alloy_network::Ethereum;
use alloy_primitives::{Address, U256};
use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::Result;

/// 1 ETH in wei.
pub const ONE_ETH: U256 = U256::from_limbs([1_000_000_000_000_000_000u64, 0, 0, 0]);

/// Set an account's native-token balance.
pub async fn set_balance(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    account: Address,
    wei: U256,
) -> Result<()> {
    provider.anvil_set_balance(account, wei).await?;
    Ok(())
}

/// Fund an account with 1 ETH and start impersonating it.
///
/// Caller is responsible for calling [`stop_impersonating`] when done.
pub async fn impersonate_funded(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    account: Address,
) -> Result<()> {
    provider.anvil_set_balance(account, ONE_ETH).await?;
    provider.anvil_impersonate_account(account).await?;
    tracing::debug!(%account, "impersonate_funded");
    Ok(())
}

/// Enable auto-impersonation for the lifetime of the test.
///
/// After this call, any `from` field on a transaction request is honored without a
/// signer. Use sparingly — it removes the safety of explicit impersonation.
pub async fn enable_auto_impersonate(
    provider: &(impl Provider + AnvilApi<Ethereum>),
) -> Result<()> {
    provider.anvil_auto_impersonate_account(true).await?;
    Ok(())
}

/// Stop impersonating an account.
pub async fn stop_impersonating(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    account: Address,
) -> Result<()> {
    provider.anvil_stop_impersonating_account(account).await?;
    Ok(())
}
