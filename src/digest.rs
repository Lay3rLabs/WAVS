use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use sha2::{Digest as Sha2Digest, Sha256};
use std::{fmt, str::FromStr};
use thiserror::Error;

// TODO: make this only one algorithm, so it is clear.
// Otherwise we have multiple digests for the same data

/// Computed content digest.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Digest {
    Sha256([u8; 32]),
}

impl Digest {
    pub fn hex_encoded(&self) -> String {
        match self {
            Digest::Sha256(digest) => hex::encode(digest.as_slice()),
        }
    }

    pub fn new_sha_256(bytes: &[u8]) -> Self {
        let mut digest = [0u8; 32];
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize_into((&mut digest).into());
        Digest::Sha256(digest)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Digest::Sha256(digest) => write!(f, "sha256:{}", hex::encode(digest.as_slice())),
        }
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Digest::Sha256(digest) => write!(f, "sha256:{}", hex::encode(digest.as_slice())),
        }
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (algo_part, bytes_part) = s
            .split_once(':')
            .ok_or_else(|| DigestError::IncorrectStructure(s.matches(':').count() + 1))?;

        match algo_part {
            "sha256" => {
                let mut bytes = [0u8; 32];
                hex::decode_to_slice(bytes_part, &mut bytes)?;
                Ok(Digest::Sha256(bytes))
            }
            _ => Err(DigestError::InvalidHashAlgorithm(algo_part.to_string())),
        }
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

impl<'de> Visitor<'de> for StrVisitor {
    type Value = Digest;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("expected hex-encoded string with a prefix for the hash algorithm type")
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
    #[error("expected two parts for hash; found {0}")]
    IncorrectStructure(usize),

    #[error("unable to parse hash algorithm: {0}")]
    InvalidHashAlgorithm(String),

    #[error("hexadecimal decode failed: {0}")]
    InvalidHex(#[from] hex::FromHexError),
}
