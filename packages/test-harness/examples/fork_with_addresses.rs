//! Fork-tier example — load the Base profile, log addresses, never leak the RPC URL.
//!
//! Run with:
//!   `FORK_RPC_URL=<your-base-rpc> cargo run -p wavs-test-harness \
//!        --example fork_with_addresses --features fork`
//!
//! This example does NOT spawn the fork — it only demonstrates loading the
//! profile, redacted logging, and address lookup. Spawning is a one-liner via
//! `chain::spawn_fork(ForkOptions::from_env(...))`.

use wavs_test_harness::{chain, fixtures::ChainProfile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let profile = ChainProfile::load("base")?;
    println!("[example] profile: name={} chain_id={}",
        profile.chain.name, profile.chain.chain_id);

    // The RPC URL is read from FORK_RPC_URL and never printed verbatim.
    match profile.resolve_rpc_url() {
        Ok(Some(url)) => println!(
            "[example] fork RPC ready (redacted): {}",
            chain::redact_url(&url)
        ),
        Ok(None) => println!("[example] profile does not declare an rpc_env"),
        Err(e) => {
            eprintln!("[example] FORK_RPC_URL unset: {e}");
            eprintln!("[example] set it and re-run to actually spawn the fork");
            return Ok(());
        }
    }

    // Address lookup demo.
    let usdc = profile.address("usdc")?;
    let weth = profile.address("weth")?;
    let avantis = profile.address("avantis_trading")?;
    let whale = profile.accounts.address("usdc_whale")?;
    println!("[example] USDC          = {usdc}");
    println!("[example] WETH          = {weth}");
    println!("[example] Avantis Trade = {avantis}");
    println!("[example] USDC whale    = {whale}");

    Ok(())
}
