//! Smoke tests for the chain control layer against a local Anvil.
//!
//! These tests do not require any external RPC and should run in any CI tier.

use alloy_primitives::{address, U256};
use alloy_provider::Provider;
use wavs_test_harness::chain;

#[tokio::test]
async fn spawn_local_anvil_round_trip() {
    let (provider, _anvil) = chain::spawn_local().await.expect("spawn local");
    let block = provider.get_block_number().await.expect("block_number");
    assert_eq!(block, 0, "fresh anvil should be at block 0");
}

#[tokio::test]
async fn mine_blocks_advances_chain() {
    let (provider, _anvil) = chain::spawn_local().await.unwrap();
    chain::mine_blocks(&provider, 5).await.unwrap();
    let block = provider.get_block_number().await.unwrap();
    assert_eq!(block, 5);
}

#[tokio::test]
async fn snapshot_revert_round_trip() {
    let (provider, _anvil) = chain::spawn_local().await.unwrap();
    chain::mine_blocks(&provider, 3).await.unwrap();
    let snap = chain::SnapshotGuard::take(&provider).await.unwrap();
    chain::mine_blocks(&provider, 10).await.unwrap();
    let before_revert = provider.get_block_number().await.unwrap();
    assert_eq!(before_revert, 13);
    assert!(snap.revert(&provider).await.unwrap());
    let after_revert = provider.get_block_number().await.unwrap();
    assert_eq!(after_revert, 3, "should revert to snapshot block");
}

#[tokio::test]
async fn impersonate_and_set_balance() {
    let (provider, _anvil) = chain::spawn_local().await.unwrap();
    let alice = address!("00000000000000000000000000000000000000aa");

    chain::set_balance(&provider, alice, U256::from(42u64) * chain::ONE_ETH)
        .await
        .unwrap();
    let bal = provider.get_balance(alice).await.unwrap();
    assert_eq!(bal, U256::from(42u64) * chain::ONE_ETH);

    chain::impersonate_funded(&provider, alice).await.unwrap();
    chain::stop_impersonating(&provider, alice).await.unwrap();
}

#[test]
fn fork_options_from_env_requires_fork_rpc_url() {
    // With no FORK_RPC_URL set, from_env must fail with a non-leaking error.
    temp_env::with_var_unset("FORK_RPC_URL", || {
        let res = wavs_test_harness::chain::ForkOptions::from_env();
        assert!(res.is_err());
        let msg = format!("{}", res.unwrap_err());
        assert!(msg.contains("FORK_RPC_URL"));
        // The error must not leak any URL — only the env var name.
        assert!(!msg.contains("https://"));
    });
}
