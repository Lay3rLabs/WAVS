use std::{str::FromStr, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
#[cfg(feature = "ts-bindings")]
use ts_rs::TS;
use utoipa::ToSchema;

/// A `ChainKey` represents a blockchain network identifier, consisting of a namespace and a chain id.
/// The namespace indicates the type of blockchain (e.g., "ethereum", "bitcoin"),
/// while the id specifies the particular network within that namespace (e.g., "1", "cosmoshub").
///
/// Mostly follows the specification at https://chainagnostic.org/CAIPs/caip-2
/// but allows the namespace part to 1 to 32 characters instead of 3 to 8
/// and changes the naming of chain_id -> chain_key, reference -> chain_id
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export, type = "string"))]
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ToSchema, bincode::Decode, bincode::Encode,
)]
pub struct ChainKey {
    pub namespace: ChainKeyNamespace,
    pub id: ChainKeyId,
}

type ChainKeyResult<T> = Result<T, ChainKeyError>;

static CHAIN_KEY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([-a-z0-9]{1,32}):([-_a-zA-Z0-9]{1,32})$").unwrap());

static NAMESPACE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-a-z0-9]{1,32}$").unwrap());

static ID_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-_a-zA-Z0-9]{1,32}$").unwrap());

impl ChainKey {
    /// Validates without taking ownership - good for checking
    pub fn validate(s: impl AsRef<str>) -> ChainKeyResult<()> {
        let s = s.as_ref();
        if !CHAIN_KEY_REGEX.is_match(s) {
            Err(ChainKeyError::FormatError)
        } else {
            Ok(())
        }
    }

    /// Construct a new validated `ChainKey`
    pub fn new(s: impl Into<String>) -> ChainKeyResult<Self> {
        let s = s.into();
        let captures = CHAIN_KEY_REGEX
            .captures(&s)
            .ok_or(ChainKeyError::FormatError)?;

        let namespace = ChainKeyNamespace::new(&captures[1])?;
        let id = ChainKeyId::new(&captures[2])?;

        Ok(Self { namespace, id })
    }
}

impl<'de> Deserialize<'de> for ChainKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ChainKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl std::fmt::Display for ChainKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.id)
    }
}

impl TryFrom<&str> for ChainKey {
    type Error = ChainKeyError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl FromStr for ChainKey {
    type Err = ChainKeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl From<ChainKey> for layer_climb_config::ChainId {
    fn from(key: ChainKey) -> Self {
        // climb doesn't care about the namespace at all
        layer_climb_config::ChainId::new(key.id.into_inner())
    }
}

// ----------------------------
// ChainKeyNamespace
// ----------------------------
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ToSchema, bincode::Decode, bincode::Encode,
)]
pub struct ChainKeyNamespace(String);

impl ChainKeyNamespace {
    pub const EVM: &str = "evm";
    pub const COSMOS: &str = "cosmos";
    pub const DEV: &str = "dev";

    pub fn new(s: impl Into<String>) -> ChainKeyResult<Self> {
        let s = s.into();
        if NAMESPACE_REGEX.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(ChainKeyError::InvalidNamespace)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for ChainKeyNamespace {
    type Err = ChainKeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<&str> for ChainKeyNamespace {
    type Error = ChainKeyError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl std::fmt::Display for ChainKeyNamespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ChainKeyNamespace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ChainKeyNamespace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

// ----------------------------
// ChainKeyId
// ----------------------------
#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ToSchema, bincode::Decode, bincode::Encode,
)]
pub struct ChainKeyId(String);

impl ChainKeyId {
    pub fn new(s: impl Into<String>) -> ChainKeyResult<Self> {
        let s = s.into();
        if ID_REGEX.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(ChainKeyError::InvalidId)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for ChainKeyId {
    type Err = ChainKeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<&str> for ChainKeyId {
    type Error = ChainKeyError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl std::fmt::Display for ChainKeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ChainKeyId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ChainKeyId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl From<ChainKeyId> for layer_climb_config::ChainId {
    fn from(id: ChainKeyId) -> Self {
        layer_climb_config::ChainId::new(id.into_inner())
    }
}

// ----------------------------
// Errors
// ----------------------------
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ChainKeyError {
    #[error("ChainKey must follow the CAIP-2-like format of 'namespace:id'")]
    FormatError,
    #[error("Invalid namespace component")]
    InvalidNamespace,
    #[error("Invalid id component")]
    InvalidId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_chain_ids() {
        assert!(ChainKey::new("eip155:1").is_ok());
        assert!(ChainKey::new("cosmos:cosmoshub-4").is_ok());
        assert!(ChainKey::new("polkadot:b0a8d493285c2df73290dfb7e61f870f").is_ok());
        assert!(ChainKey::new("abc:X_Y-Z123").is_ok());
        assert!(ChainKey::new("abc-123:ABC_xyz-123").is_ok());
    }

    #[test]
    fn test_invalid_chain_ids() {
        // Invalid format
        assert!(ChainKey::new("no-colon").is_err());
        assert!(ChainKey::new("too:many:colons").is_err());
        assert!(ChainKey::new(":empty-namespace").is_err());
        assert!(ChainKey::new("empty-id:").is_err());

        // Invalid namespace
        assert!(ChainKey::new("thisiswaytoolongtobeavalidnamespace:ref").is_err()); // too long
        assert!(ChainKey::new("ABC:ref").is_err()); // uppercase not allowed
        assert!(ChainKey::new("ab_c:ref").is_err()); // underscore not allowed in namespace
        assert!(ChainKey::new("ab.c:ref").is_err()); // dot not allowed

        // Invalid id
        assert!(ChainKey::new("abc:").is_err()); // empty reference
        assert!(ChainKey::new("abc:this-is-way-too-long-to-be-a-valid-idt").is_err()); // too long (33 chars)
        assert!(ChainKey::new("abc:id.with.dots").is_err()); // dots not allowed
        assert!(ChainKey::new("abc:id@123").is_err()); // @ not allowed
        assert!(ChainKey::new("abc:id space").is_err()); // space not allowed
    }

    #[test]
    fn test_accessors() {
        let chain_key = ChainKey::new("eip155:1").unwrap();
        assert_eq!(chain_key.to_string(), "eip155:1");
        assert_eq!(chain_key.namespace.as_str(), "eip155");
        assert_eq!(chain_key.id.as_str(), "1");
    }

    #[test]
    fn test_edge_cases() {
        // Minimum valid lengths
        assert!(ChainKey::new("abc:x").is_ok());

        // Maximum valid lengths
        assert!(ChainKey::new("abcd1234:ABCD1234_abcd1234-ABCD1234").is_ok());

        // Case sensitivity is preserved
        let chain_key = ChainKey::new("eip155:MyChain_123").unwrap();
        assert_eq!(chain_key.id.as_str(), "MyChain_123");
    }
}
