//! [`ChainProfile`] — TOML-backed description of a chain (local, base, mainnet, …).
//!
//! Format mirrors the schema in issue #1147:
//!
//! ```toml
//! [chain]
//! name = "base"
//! chain_id = 8453
//! fork_block = 29000000
//! rpc_env = "FORK_RPC_URL"
//!
//! [addresses]
//! usdc = "0x..."
//!
//! [accounts]
//! funded_key_env = "FUNDED_KEY"
//! usdc_whale = "0x..."
//! ```
//!
//! Three profiles ship with the crate: `local`, `base`, `mainnet`. Consumers may
//! also load arbitrary profiles via [`ChainProfile::from_path`] or
//! [`ChainProfile::from_str`].

use std::path::Path;

use alloy_primitives::Address;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use super::addresses::Addresses;

/// Top-level `[chain]` table.
#[derive(Debug, Clone, Deserialize)]
pub struct ChainSection {
    pub name: String,
    pub chain_id: u64,
    #[serde(default)]
    pub fork_block: Option<u64>,
    #[serde(default)]
    pub rpc_env: Option<String>,
}

/// Top-level `[accounts]` table. `funded_key_env` names the env var that holds the
/// deployer private key. Additional named accounts (whales, governance addresses)
/// are exposed as a flat lookup via [`AccountsSection::address`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountsSection {
    #[serde(default)]
    pub funded_key_env: Option<String>,
    #[serde(flatten, default)]
    pub addresses: Addresses,
}

impl AccountsSection {
    /// Look up a named account address (e.g. `usdc_whale`, `avantis_gov`).
    pub fn address(&self, name: &str) -> Result<Address> {
        self.addresses.require(name)
    }
}

/// A complete chain profile loaded from a TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct ChainProfile {
    pub chain: ChainSection,
    #[serde(default)]
    pub addresses: Addresses,
    #[serde(default)]
    pub accounts: AccountsSection,
}

const LOCAL_TOML: &str = include_str!("../../fixtures/chains/local.toml");
const BASE_TOML: &str = include_str!("../../fixtures/chains/base.toml");
const MAINNET_TOML: &str = include_str!("../../fixtures/chains/mainnet.toml");

impl ChainProfile {
    /// Load a bundled profile by name (`local`, `base`, `mainnet`).
    pub fn load(name: &str) -> Result<Self> {
        let raw = match name {
            "local" => LOCAL_TOML,
            "base" => BASE_TOML,
            "mainnet" => MAINNET_TOML,
            other => {
                return Err(anyhow!(
                    "unknown bundled profile `{other}` — known: local, base, mainnet"
                ))
            }
        };
        Self::from_str(raw).with_context(|| format!("parse bundled profile `{name}`"))
    }

    /// Load a profile from an arbitrary path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read profile {}", path.display()))?;
        Self::from_str(&raw).with_context(|| format!("parse profile {}", path.display()))
    }

    /// Parse a profile from a TOML string. Prefer [`Self::load`] or [`Self::from_path`].
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Resolve the fork RPC URL from the env var declared in `chain.rpc_env`.
    ///
    /// Returns `None` if `rpc_env` is unset on the profile. Returns `Err` if the env
    /// var is named but missing or empty.
    pub fn resolve_rpc_url(&self) -> Result<Option<String>> {
        let Some(var) = &self.chain.rpc_env else {
            return Ok(None);
        };
        let val = std::env::var(var).with_context(|| format!("{var} must be set"))?;
        if val.is_empty() {
            return Err(anyhow!("{var} is empty"));
        }
        Ok(Some(val))
    }

    /// Convenience accessor for a named protocol address.
    pub fn address(&self, name: &str) -> Result<Address> {
        self.addresses.require(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_profile_loads() {
        let p = ChainProfile::load("local").expect("local");
        assert_eq!(p.chain.name, "local");
        assert_eq!(p.chain.chain_id, 31337);
        assert!(p.chain.rpc_env.is_none());
    }

    #[test]
    fn base_profile_has_known_addresses() {
        let p = ChainProfile::load("base").expect("base");
        assert_eq!(p.chain.name, "base");
        assert_eq!(p.chain.chain_id, 8453);
        assert_eq!(p.chain.rpc_env.as_deref(), Some("FORK_RPC_URL"));

        let usdc = p.address("usdc").expect("usdc address");
        let weth = p.address("weth").expect("weth address");
        assert_ne!(usdc, weth);

        let whale = p.accounts.address("usdc_whale").expect("usdc_whale");
        assert_ne!(whale, Address::ZERO);
    }

    #[test]
    fn mainnet_profile_loads() {
        let p = ChainProfile::load("mainnet").expect("mainnet");
        assert_eq!(p.chain.chain_id, 1);
    }

    #[test]
    fn unknown_profile_errors() {
        let e = ChainProfile::load("unknown").unwrap_err();
        assert!(format!("{e}").contains("unknown"));
    }

    #[test]
    fn resolve_rpc_url_reads_env() {
        let p = ChainProfile::load("base").unwrap();
        temp_env::with_var("FORK_RPC_URL", Some("https://api.example.com/k"), || {
            let url = p.resolve_rpc_url().unwrap();
            assert_eq!(url.as_deref(), Some("https://api.example.com/k"));
        });
    }

    #[test]
    fn resolve_rpc_url_errors_if_missing() {
        let p = ChainProfile::load("base").unwrap();
        temp_env::with_var_unset("FORK_RPC_URL", || {
            let res = p.resolve_rpc_url();
            assert!(res.is_err());
        });
    }
}
