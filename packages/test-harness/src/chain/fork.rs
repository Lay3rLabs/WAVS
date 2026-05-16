//! Pinned mainnet-fork spawn for fork-tier tests.
//!
//! The fork URL and (optionally) pinned block number are read either from an explicit
//! [`ForkOptions`] struct or from the `FORK_RPC_URL` / `FORK_BLOCK_NUMBER` env vars.
//! The URL is never logged verbatim — only a redacted suffix via
//! [`crate::chain::logging::redact_url`].
//!
//! Pinned block selection follows this precedence (highest wins):
//!
//! 1. [`ForkOptions::block_number`] set explicitly by the caller.
//! 2. `FORK_BLOCK_NUMBER` env var (decimal `u64`).
//! 3. [`crate::fixtures::ChainProfile::chain.fork_block`] via
//!    [`ForkOptions::from_profile`].
//!
//! If none of these resolve, the fork runs against the upstream's latest block
//! and the spawn emits a `warn!` so the determinism gap is visible in logs.
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

#[cfg(feature = "fork")]
pub const DEFAULT_FORK_BLOCK_ENV: &str = "FORK_BLOCK_NUMBER";

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
    /// Build options from the environment.
    ///
    /// Reads:
    /// - `FORK_RPC_URL` — required. Returns `Err` if unset or empty.
    /// - `FORK_BLOCK_NUMBER` — optional. If set, must parse as `u64`.
    ///
    /// The caller may override the block number by passing
    /// [`ForkOptions { block_number: Some(_), .. }`] after merging via
    /// [`Self::with_block_number`].
    pub fn from_env() -> Result<Self> {
        let rpc_url = std::env::var(DEFAULT_FORK_RPC_ENV)
            .with_context(|| format!("{DEFAULT_FORK_RPC_ENV} must be set for fork-tier tests"))?;
        if rpc_url.is_empty() {
            return Err(anyhow!("{DEFAULT_FORK_RPC_ENV} is empty"));
        }
        let block_number = block_number_from_env()?;
        Ok(Self {
            rpc_url: Some(rpc_url),
            block_number,
            timeout_ms: None,
        })
    }

    /// Build options from a [`ChainProfile`](crate::fixtures::ChainProfile).
    ///
    /// Resolves the RPC URL through the profile's `rpc_env` declaration
    /// (typically `FORK_RPC_URL`). The pinned block follows this precedence:
    ///
    /// 1. `FORK_BLOCK_NUMBER` env var if set (CI override).
    /// 2. `chain.fork_block` declared in the profile.
    ///
    /// Returns `Err` if the profile does not declare an `rpc_env` or if the
    /// declared env var is unset.
    pub fn from_profile(profile: &crate::fixtures::ChainProfile) -> Result<Self> {
        let rpc_url = profile
            .resolve_rpc_url()?
            .ok_or_else(|| anyhow!("profile `{}` has no rpc_env declared", profile.chain.name))?;
        let block_number = block_number_from_env()?.or(profile.chain.fork_block);
        Ok(Self {
            rpc_url: Some(rpc_url),
            block_number,
            timeout_ms: None,
        })
    }

    /// Override the pinned block number.
    pub fn with_block_number(mut self, block_number: u64) -> Self {
        self.block_number = Some(block_number);
        self
    }

    /// Override the Anvil spawn timeout.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Read `FORK_BLOCK_NUMBER` from the environment if set. Returns `Err` if set
/// but not parseable as a `u64`.
fn block_number_from_env() -> Result<Option<u64>> {
    match std::env::var(DEFAULT_FORK_BLOCK_ENV) {
        Ok(v) if !v.is_empty() => v
            .parse::<u64>()
            .map(Some)
            .with_context(|| format!("{DEFAULT_FORK_BLOCK_ENV} must be a decimal u64, got `{v}`")),
        _ => Ok(None),
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
        None => tracing::warn!(
            rpc = %redacted,
            "spawning forked anvil at LATEST upstream block — fork-tier tests are non-deterministic unless FORK_BLOCK_NUMBER or ChainProfile.fork_block is set"
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_requires_rpc_url() {
        temp_env::with_var_unset(DEFAULT_FORK_RPC_ENV, || {
            let res = ForkOptions::from_env();
            assert!(res.is_err());
            let msg = format!("{}", res.unwrap_err());
            assert!(msg.contains(DEFAULT_FORK_RPC_ENV), "msg = {msg}");
        });
    }

    #[test]
    fn from_env_rejects_empty_rpc_url() {
        temp_env::with_var(DEFAULT_FORK_RPC_ENV, Some(""), || {
            let res = ForkOptions::from_env();
            assert!(res.is_err());
        });
    }

    #[test]
    fn from_env_reads_block_number_when_set() {
        temp_env::with_vars(
            [
                (DEFAULT_FORK_RPC_ENV, Some("https://example.com/k")),
                (DEFAULT_FORK_BLOCK_ENV, Some("29000000")),
            ],
            || {
                let opts = ForkOptions::from_env().unwrap();
                assert_eq!(opts.block_number, Some(29_000_000));
            },
        );
    }

    #[test]
    fn from_env_block_number_unset_is_none() {
        temp_env::with_vars(
            [
                (DEFAULT_FORK_RPC_ENV, Some("https://example.com/k")),
                (DEFAULT_FORK_BLOCK_ENV, None),
            ],
            || {
                let opts = ForkOptions::from_env().unwrap();
                assert_eq!(opts.block_number, None);
            },
        );
    }

    #[test]
    fn from_env_block_number_invalid_errors() {
        temp_env::with_vars(
            [
                (DEFAULT_FORK_RPC_ENV, Some("https://example.com/k")),
                (DEFAULT_FORK_BLOCK_ENV, Some("not-a-number")),
            ],
            || {
                let res = ForkOptions::from_env();
                assert!(res.is_err());
                let msg = format!("{}", res.unwrap_err());
                assert!(msg.contains(DEFAULT_FORK_BLOCK_ENV), "msg = {msg}");
            },
        );
    }

    #[test]
    fn from_profile_prefers_env_block_over_profile_block() {
        let profile = crate::fixtures::ChainProfile::load("base").unwrap();
        let profile_block = profile.chain.fork_block;
        assert!(
            profile_block.is_some(),
            "base profile must declare fork_block"
        );

        temp_env::with_vars(
            [
                (DEFAULT_FORK_RPC_ENV, Some("https://example.com/k")),
                (DEFAULT_FORK_BLOCK_ENV, Some("42")),
            ],
            || {
                let opts = ForkOptions::from_profile(&profile).unwrap();
                assert_eq!(opts.block_number, Some(42));
            },
        );
    }

    #[test]
    fn from_profile_falls_back_to_profile_block_when_env_unset() {
        let profile = crate::fixtures::ChainProfile::load("base").unwrap();
        let expected = profile.chain.fork_block;
        assert!(expected.is_some());

        temp_env::with_vars(
            [
                (DEFAULT_FORK_RPC_ENV, Some("https://example.com/k")),
                (DEFAULT_FORK_BLOCK_ENV, None),
            ],
            || {
                let opts = ForkOptions::from_profile(&profile).unwrap();
                assert_eq!(opts.block_number, expected);
            },
        );
    }
}
