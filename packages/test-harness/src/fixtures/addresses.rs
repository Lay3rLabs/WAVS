//! Typed address lookup keyed by symbolic name.
//!
//! Wraps a `BTreeMap<String, Address>` so iteration order is deterministic. Addresses
//! parse from standard checksummed or lowercase hex (alloy `Address` `FromStr`).

use std::collections::BTreeMap;

use alloy_primitives::Address;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// A flat name → address lookup table loaded from a TOML `[addresses]` or
/// `[accounts]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Addresses(BTreeMap<String, Address>);

impl Addresses {
    /// Look up an address by symbolic name (case-sensitive).
    pub fn get(&self, name: &str) -> Option<Address> {
        self.0.get(name).copied()
    }

    /// Look up an address or return a descriptive error.
    pub fn require(&self, name: &str) -> Result<Address> {
        self.get(name).ok_or_else(|| {
            let mut keys: Vec<&str> = self.0.keys().map(String::as_str).collect();
            keys.sort();
            anyhow!(
                "no address named `{name}` in profile; known: [{}]",
                keys.join(", ")
            )
        })
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Address)> {
        self.0.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Total number of addresses.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True iff no addresses are present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
