use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use sha2::{Digest as Sha2Digest, Sha256};
use std::{fmt, str::FromStr};
use thiserror::Error;
use utoipa::ToSchema;

/// Computed content digest. Set to Sha256, but we can change globally in this file
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
pub struct Digest([u8; 32]);

impl Digest {
    pub fn hex_encoded(&self) -> String {
        const_hex::encode(self.0.as_slice())
    }

    pub fn new(bytes: &[u8]) -> Self {
        let mut digest = [0u8; 32];
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize_into((&mut digest).into());
        Digest(digest)
    }
}

impl From<[u8; 32]> for Digest {
    fn from(value: [u8; 32]) -> Self {
        Digest(value)
    }
}

impl AsRef<[u8]> for Digest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.hex_encoded())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.hex_encoded())
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        const_hex::decode_to_slice(s, &mut bytes)?;
        Ok(Digest(bytes))
    }
}

impl Serialize for Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct StrVisitor;

impl Visitor<'_> for StrVisitor {
    type Value = Digest;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("expected hex-encoded string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Digest::from_str(value).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D>(deserializer: D) -> Result<Digest, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StrVisitor)
    }
}

/// Parsing errors from the string-encoded digest.
#[derive(Error, Debug)]
pub enum DigestError {
    #[error("hexadecimal decode failed: {0}")]
    InvalidHex(#[from] const_hex::FromHexError),
}
