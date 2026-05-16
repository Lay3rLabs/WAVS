//! Oracle-mocking smoke tests.
//!
//! Demonstrates the `anvil_setCode` pattern for installing a deterministic
//! oracle reading at a production address — the exact thing downstream apps
//! like `wavs-defi` need to drive their delta/health-check logic.
//!
//! - `chainlink_at_arbitrary_address_local`: local Anvil, mock at a fresh address.
//! - `chainlink_at_base_eth_usd_fork`: fork-tier, replaces the live Base ETH/USD
//!   feed. Skipped unless `FORK_RPC_URL` (and optionally `FORK_BLOCK_NUMBER`)
//!   are set.

use alloy_primitives::{Address, I256};

use wavs_test_harness::{
    chain::{
        self,
        oracle::{
            chainlink_usd, install_chainlink_aggregator_v3, set_chainlink_price, IChainlinkV3Mock,
        },
    },
    fixtures::ChainProfile,
};

#[tokio::test]
async fn chainlink_at_arbitrary_address_local() {
    let _ = tracing_subscriber::fmt::try_init();

    let (provider, _anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn anvil with deployer");

    let feed = Address::from([0xc1; 20]);
    install_chainlink_aggregator_v3(&provider, feed, chainlink_usd(3_000.0))
        .await
        .expect("install chainlink mock");

    let mock = IChainlinkV3Mock::new(feed, &provider);
    let r = mock.latestRoundData().call().await.unwrap();
    assert_eq!(r._1, chainlink_usd(3_000.0));

    // Drive a price-shock scenario.
    set_chainlink_price(&provider, feed, chainlink_usd(2_700.0))
        .await
        .unwrap();
    let r = mock.latestRoundData().call().await.unwrap();
    assert_eq!(r._1, chainlink_usd(2_700.0));
    assert_eq!(mock.decimals().call().await.unwrap(), 8);
}

#[tokio::test]
async fn chainlink_at_base_eth_usd_fork() {
    let _ = tracing_subscriber::fmt::try_init();

    if std::env::var("FORK_RPC_URL").is_err() {
        eprintln!("[skipping] FORK_RPC_URL not set");
        return;
    }

    let profile = ChainProfile::load("base").expect("load base profile");
    let opts = chain::ForkOptions::from_profile(&profile).expect("from_profile");
    let (provider, _anvil) = chain::spawn_fork(opts).await.expect("spawn base fork");

    // We need a wallet-bound provider for the setPrice call to be signed.
    // For fork tests, easiest is to also spawn a wallet via the same chain.
    let chainlink = profile.address("chainlink_eth_usd").expect("chainlink");

    // Capture a real read first to prove we're on a live fork.
    let real = IChainlinkV3Mock::new(chainlink, &provider)
        .latestRoundData()
        .call()
        .await
        .expect("read real chainlink");
    assert!(real._1 > I256::ZERO, "live chainlink feed must be positive");
    let real_price = real._1;
    eprintln!("[fork] live ETH/USD before install: {real_price}");

    // anvil_set_code doesn't need signing — install the mock, then we'll use
    // an impersonated anvil deployer to setPrice.
    install_chainlink_aggregator_v3(&provider, chainlink, chainlink_usd(123.45))
        .await
        .expect("install mock on live chainlink");

    // Confirm the read shape now returns our value.
    let after = IChainlinkV3Mock::new(chainlink, &provider)
        .latestRoundData()
        .call()
        .await
        .expect("read mocked chainlink");
    assert_eq!(after._1, chainlink_usd(123.45));
    // The mock's latestRoundData returns roundId = 1.
    assert_eq!(after._0, alloy_primitives::aliases::U80::from(1u64));
}
