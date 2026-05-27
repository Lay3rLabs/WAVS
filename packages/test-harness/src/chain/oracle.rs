//! Oracle mocking helpers for forked-chain tests.
//!
//! Downstream apps (`wavs-defi`, `wavs-aave-guardian`, …) read prices from
//! production oracle contracts — Chainlink for ETH/USD on Base, Pyth for
//! Avantis settlement, and so on. A reproducible integration test needs to
//! drive those reads with predictable values without leaving the fork.
//!
//! The cheapest path is `anvil_setCode`: replace the runtime bytecode at the
//! oracle address with a minimal mock that exposes a `setPrice(int256)`
//! setter. After install, any contract reading `latestRoundData()` on that
//! address sees the mock's value. The production address stays unchanged, so
//! contract code paths that hard-code it (and there are many — see
//! `wavs-defi`'s `SmartVaultStorage`) keep working untouched.
//!
//! v1 ships a Chainlink `AggregatorV3Interface` mock. Pyth + other oracle
//! shapes are tracked as a follow-up — the same `anvil_set_code` pattern
//! applies; only the bytecode differs.

use alloy_network::Ethereum;
use alloy_primitives::{hex, Address, Bytes, I256};
use alloy_provider::{ext::AnvilApi, Provider};
use alloy_sol_types::{sol, SolCall};
use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Chainlink AggregatorV3 mock — replaces the runtime code at the live feed.
//
// Equivalent to `wavs-defi`'s MockChainlink.sol: stored `int256 price`,
// `setPrice(int256)`, `latestRoundData() -> (roundId, answer, startedAt,
// updatedAt, answeredInRound)` with answer = stored price, timestamps =
// block.timestamp, roundId = 1. `decimals()` returns 8 to match production
// Chainlink feeds. Compiled with solc 0.8.27, runtime bytecode embedded
// below as a hex string — no `solc` / `forge` step required at harness
// build time.
// ---------------------------------------------------------------------------

const CHAINLINK_V3_MOCK_RUNTIME: &str = "6080806040526004361015610012575f80fd5b5f3560e01c908163313ce56714610145575080637284e416146100c1578063a035b1fe146100a5578063f7a308061461008d5763feaf968c14610053575f80fd5b34610089575f3660031901126100895760a05f546040519060018252602082015242604082015242606082015260016080820152f35b5f80fd5b34610089576020366003190112610089576004355f55005b34610089575f3660031901126100895760205f54604051908152f35b34610089575f366003190112610089576040516040810181811067ffffffffffffffff821117610131576040526007815260406020820191661155120bd554d160ca1b83528151928391602083525180918160208501528484015e5f828201840152601f01601f19168101030190f35b634e487b7160e01b5f52604160045260245ffd5b34610089575f3660031901126100895780600860209252f3fea2646970667358221220888eeb3f5309afd8589b5038c99978400f4b86c23441180cdf4dd21be69c3af164736f6c634300081b0033";

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IChainlinkV3Mock {
        function setPrice(int256 _price) external;
        function latestRoundData() external view returns (uint80, int256, uint256, uint256, uint80);
        function decimals() external view returns (uint8);
    }
}

/// Install a Chainlink AggregatorV3-compatible mock at `feed_address` and seed
/// it with `initial_price` (8-decimal integer, e.g. `chainlink_usd(3000.0)`).
///
/// Uses Anvil's `setCode` cheatcode — the address stays the same so downstream
/// contracts that hard-code the production feed address keep working.
///
/// # Example
///
/// ```no_run
/// use alloy_primitives::Address;
/// use wavs_test_harness::chain::oracle::{chainlink_usd, install_chainlink_aggregator_v3};
/// # async fn doctest() -> anyhow::Result<()> {
/// let (provider, _anvil) = wavs_test_harness::chain::spawn_local().await?;
/// let chainlink_eth_usd: Address = "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70".parse()?;
/// install_chainlink_aggregator_v3(&provider, chainlink_eth_usd, chainlink_usd(3_000.0)).await?;
/// # Ok(()) }
/// ```
pub async fn install_chainlink_aggregator_v3<P>(
    provider: &P,
    feed_address: Address,
    initial_price: I256,
) -> Result<()>
where
    P: Provider + AnvilApi<Ethereum>,
{
    let bytecode = hex::decode(CHAINLINK_V3_MOCK_RUNTIME)
        .context("decode embedded ChainlinkV3Mock runtime bytecode")?;
    provider
        .anvil_set_code(feed_address, Bytes::from(bytecode))
        .await
        .context("anvil_set_code on chainlink mock")?;
    set_chainlink_price(provider, feed_address, initial_price).await?;
    tracing::debug!(
        feed = %feed_address,
        price = %initial_price,
        "installed Chainlink AggregatorV3 mock"
    );
    Ok(())
}

/// Update the price reported by a previously-installed Chainlink mock. Returns
/// `Err` if the address holds something other than the mock.
pub async fn set_chainlink_price<P: Provider>(
    provider: &P,
    feed_address: Address,
    price: I256,
) -> Result<()> {
    let calldata = IChainlinkV3Mock::setPriceCall { _price: price }.abi_encode();
    let tx = alloy_rpc_types_eth::TransactionRequest::default()
        .to(feed_address)
        .input(calldata.into());
    let pending = provider
        .send_transaction(tx)
        .await
        .context("send setPrice tx")?;
    let receipt = pending
        .get_receipt()
        .await
        .context("await setPrice receipt")?;
    if !receipt.status() {
        anyhow::bail!("setPrice reverted on {feed_address} (mock not installed?)");
    }
    Ok(())
}

/// Convenience: convert a USD price in 8-decimal form (Chainlink convention)
/// to the `int256` argument format. E.g. `chainlink_usd(3_000.0)` →
/// `3000_00000000`.
pub fn chainlink_usd(price_usd: f64) -> I256 {
    let scaled = (price_usd * 1e8).round() as i128;
    I256::try_from(scaled).expect("chainlink_usd price overflow")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chainlink_usd_scales_correctly() {
        assert_eq!(
            chainlink_usd(3000.0),
            I256::try_from(300_000_000_000_i64).unwrap()
        );
        assert_eq!(chainlink_usd(0.5), I256::try_from(50_000_000_i64).unwrap());
    }

    #[tokio::test]
    async fn chainlink_mock_install_and_set_price_roundtrip() {
        let (provider, _anvil, _deployer) =
            crate::chain::spawn_local_with_deployer().await.unwrap();
        let feed = Address::from([0xab; 20]);
        install_chainlink_aggregator_v3(&provider, feed, chainlink_usd(3000.0))
            .await
            .expect("install chainlink mock");

        // Read it back through the production interface.
        let mock = IChainlinkV3Mock::new(feed, &provider);
        let round = mock
            .latestRoundData()
            .call()
            .await
            .expect("latestRoundData");
        assert_eq!(round._1, chainlink_usd(3000.0));

        // Update via setPrice and read again.
        set_chainlink_price(&provider, feed, chainlink_usd(2500.0))
            .await
            .unwrap();
        let round = mock.latestRoundData().call().await.unwrap();
        assert_eq!(round._1, chainlink_usd(2500.0));

        // decimals() should return 8 — matches production Chainlink feeds.
        let d = mock.decimals().call().await.unwrap();
        assert_eq!(d, 8);
    }
}
