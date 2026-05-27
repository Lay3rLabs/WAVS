//! Block-time and mining control.
//!
//! Half the flake in fork-tier tests comes from "did the next block tick yet" — these
//! helpers make the answer deterministic.

use alloy_network::Ethereum;
use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::Result;

/// Mine `count` blocks immediately.
pub async fn mine_blocks(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    count: u64,
) -> Result<()> {
    provider.anvil_mine(Some(count), None).await?;
    Ok(())
}

/// Toggle auto-mining. When off, blocks are produced only via [`mine_blocks`] or
/// [`set_block_timestamp`] + a transaction.
pub async fn set_automine(provider: &(impl Provider + AnvilApi<Ethereum>), on: bool) -> Result<()> {
    provider.anvil_set_auto_mine(on).await?;
    Ok(())
}

/// Advance the chain's clock by `seconds` and mine one block to commit it.
pub async fn increase_time(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    seconds: u64,
) -> Result<()> {
    provider.anvil_increase_time(seconds).await?;
    provider.anvil_mine(Some(1), None).await?;
    Ok(())
}

/// Set the timestamp of the next block.
pub async fn set_next_block_timestamp(
    provider: &(impl Provider + AnvilApi<Ethereum>),
    ts: u64,
) -> Result<()> {
    provider.anvil_set_next_block_timestamp(ts).await?;
    Ok(())
}
