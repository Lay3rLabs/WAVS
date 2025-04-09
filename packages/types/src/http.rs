pub mod aggregator;
use super::{Permissions, ServiceID, ServiceStatus, Trigger};
use crate::{digest::Digest, ChainName};
use layer_climb_address::Address;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{fmt, ops::Deref, str::FromStr};
use wasm_pkg_common::package::{PackageRef, Version};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SigningKeyResponse {
    /// from alloy's SigningKey.to_bytes()
    Secp256k1(Vec<u8>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddServiceRequest {
    pub chain_name: ChainName,
    pub address: Address,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteServicesRequest {
    pub service_ids: Vec<ServiceID>,
}

#[derive(Serialize, Deserialize)]
pub struct ListServicesResponse {
    pub services: Vec<ListServiceResponse>,
    pub digests: Vec<ShaDigest>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListServiceResponse {
    pub id: ServiceID,
    pub status: ServiceStatus,
    pub source: ComponentSource,
    pub trigger: Trigger,
    pub permissions: Permissions,
}

#[derive(Serialize, Deserialize)]
pub struct UploadComponentResponse {
    pub digest: ShaDigest,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Registry {
    pub digest: Digest,
    /// Optional domain to use for a registry (such as ghcr.io)
    /// if default of wa.dev (or whatever wavs uses in the future)
    /// is not desired by user
    pub domain: Option<String>,
    /// Optional semver value, if absent then latest is used
    pub version: Option<Version>,
    /// Package identifier of form <namespace>:<packagename>
    pub package: PackageRef,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentSource {
    /// The wasm bytecode provided at fixed url, digest provided to ensure no tampering
    Download { url: String, digest: Digest },
    /// The wasm bytecode downloaded from a standard registry, digest provided to ensure no tampering
    Registry { registry: Registry },
    /// An already deployed component
    Digest(Digest),
}

impl ComponentSource {
    pub fn digest(&self) -> &Digest {
        match self {
            ComponentSource::Download { digest, .. } => digest,
            ComponentSource::Registry { registry } => &registry.digest,
            ComponentSource::Digest(digest) => digest,
        }
    }
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
