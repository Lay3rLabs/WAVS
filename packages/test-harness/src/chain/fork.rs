//! Pinned mainnet-fork spawn for fork-tier tests.
//!
//! The fork URL and (optionally) pinned block number are read either from an explicit
//! [`ForkOptions`] struct or from the `FORK_RPC_URL` env var. The URL is never logged
//! verbatim — only a redacted suffix via [`crate::chain::logging::redact_url`].
//!
//! Modelled after `wavs-defi/crates/integration-tests/tests/common/anvil.rs`.

use alloy_network::Ethereum;
use alloy_node_bindings::{Anvil, AnvilInstance};
use alloy_provider::{ext::AnvilApi, Provider, ProviderBuilder};
use anyhow::{anyhow, Context, Result};

use crate::chain::logging::redact_url;
use utils::test_utils::anvil::safe_spawn_anvil_extra;

#[cfg(feature = "fork")]
pub const DEFAULT_FORK_RPC_ENV: &str = "FORK_RPC_URL";

/// Options for spawning a forked Anvil instance.
#[derive(Debug, Clone, Default)]
pub struct ForkOptions {
    /// Upstream RPC URL. If `None`, reads `FORK_RPC_URL` from the environment.
    pub rpc_url: Option<String>,
    /// Pin to a specific block number. Recommended for determinism.
    pub block_number: Option<u64>,
    /// Anvil spawn timeout in milliseconds. Defaults to 60 000 ms.
    pub timeout_ms: Option<u64>,
}

impl ForkOptions {
    /// Build options from the environment (`FORK_RPC_URL`) and a pinned block.
    pub fn from_env(block_number: Option<u64>) -> Result<Self> {
        let rpc_url = std::env::var(DEFAULT_FORK_RPC_ENV)
            .with_context(|| format!("{DEFAULT_FORK_RPC_ENV} must be set for fork-tier tests"))?;
        Ok(Self {
            rpc_url: Some(rpc_url),
            block_number,
            timeout_ms: None,
        })
    }
}

/// Spawn a forked Anvil instance and return a connected provider.
///
/// Logs the RPC URL only as a redacted suffix.
#[cfg(feature = "fork")]
pub async fn spawn_fork(
    opts: ForkOptions,
) -> Result<(impl Provider + AnvilApi<Ethereum> + Clone, AnvilInstance)> {
    let rpc_url = match opts.rpc_url {
        Some(u) => u,
        None => std::env::var(DEFAULT_FORK_RPC_ENV).with_context(|| {
            format!("{DEFAULT_FORK_RPC_ENV} must be set or pass ForkOptions::rpc_url")
        })?,
    };
    if rpc_url.is_empty() {
        return Err(anyhow!("fork RPC URL is empty"));
    }
    let block_number = opts.block_number;
    let timeout_ms = opts.timeout_ms.unwrap_or(60_000);

    let redacted = redact_url(&rpc_url);
    match block_number {
        Some(b) => tracing::info!(rpc = %redacted, block = b, "spawning forked anvil"),
        None => tracing::info!(rpc = %redacted, "spawning forked anvil at latest"),
    }

    // Capture by reference / value so the retry closure stays `Fn`.
    let rpc_ref = rpc_url.as_str();
    let anvil = safe_spawn_anvil_extra(|a: Anvil| {
        let mut a = a.fork(rpc_ref).timeout(timeout_ms);
        if let Some(b) = block_number {
            a = a.fork_block_number(b);
        }
        a
    });

    tracing::info!(endpoint = %anvil.endpoint(), "anvil fork ready");

    #[allow(deprecated)]
    let provider = ProviderBuilder::new().on_http(anvil.endpoint_url());

    Ok((provider, anvil))
}
