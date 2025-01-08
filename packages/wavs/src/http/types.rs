use std::{fmt, ops::Deref, str::FromStr};

use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

use crate::Digest;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    //#[error("invalid CRON frequency")]
    //InvalidCronFrequency,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShaDigest(Digest);

impl ShaDigest {
    pub fn new(digest: Digest) -> Self {
        Self(digest)
    }
}

impl Deref for ShaDigest {
    type Target = Digest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Digest> for ShaDigest {
    fn from(digest: Digest) -> Self {
        Self(digest)
    }
}

impl From<ShaDigest> for Digest {
    fn from(digest: ShaDigest) -> Self {
        digest.0
    }
}

impl fmt::Display for ShaDigest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "sha256:{}", self.0)
    }
}

impl fmt::Debug for ShaDigest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "sha256:{:?}", self.0)
    }
}

impl Serialize for ShaDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct StrVisitor;

impl Visitor<'_> for StrVisitor {
    type Value = ShaDigest;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("expected hex-encoded string with sha256: prefix")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !value.starts_with("sha256:") {
            return Err(de::Error::custom("expected sha256: prefix"));
        }
        let value = &value[7..];
        let d = Digest::from_str(value).map_err(de::Error::custom)?;
        Ok(ShaDigest::new(d))
    }
}

impl<'de> Deserialize<'de> for ShaDigest {
    fn deserialize<D>(deserializer: D) -> Result<ShaDigest, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StrVisitor)
    }
}
