use std::{str::FromStr, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// A `ChainKey` represents a blockchain network identifier, consisting of a namespace and a chain id.
/// The namespace indicates the type of blockchain (e.g., "ethereum", "bitcoin"),
/// while the id specifies the particular network within that namespace (e.g., "1", "cosmoshub").
/// 
/// Mostly follows the specification at https://chainagnostic.org/CAIPs/caip-2 
/// but allows the namespace part to 1 to 32 characters instead of 3 to 8
/// and changes the naming of chain_id -> chain_key, reference -> chain_id
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ToSchema,
    bincode::Decode,
    bincode::Encode,
)]
pub struct ChainKey {
    pub namespace: ChainKeyNamespace,
    pub id: ChainKeyId,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ToSchema,
    bincode::Decode,
    bincode::Encode,
)]
pub struct ChainKeyNamespace(String);

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ToSchema,
    bincode::Decode,
    bincode::Encode,
)]
pub struct ChainKeyId(String);

type ChainKeyResult<T> = Result<T, ChainKeyError>;

static CHAIN_KEY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([-a-z0-9]{1,32}):([-_a-zA-Z0-9]{1,32})$").unwrap()
});

static NAMESPACE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-a-z0-9]{1,32}$").unwrap());

static ID_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-_a-zA-Z0-9]{1,32}$").unwrap());

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

impl From<ChainKey> for layer_climb::prelude::ChainId {
    fn from(key: ChainKey) -> Self {
        // climb doesn't care about the namespace at all
        layer_climb::prelude::ChainId::new(key.id.into_inner())
    }
}

// ----------------------------
// ChainKeyNamespace
// ----------------------------
impl ChainKeyNamespace {
    pub const EVM:&str = "evm";
    pub const COSMOS:&str = "cosmos";
    pub const DEV:&str = "dev";

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
