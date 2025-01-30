use std::{fmt, ops::Deref, str::FromStr};

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::digest::Digest;

use super::{Permissions, Service, ServiceID, ServiceStatus, Trigger, TriggerData};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddServiceRequest {
    pub service: Service,
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
    pub digest: ShaDigest,
    pub trigger: Trigger,
    pub permissions: Permissions,
    pub testable: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct UploadServiceResponse {
    pub digest: ShaDigest,
}

#[derive(Serialize, Deserialize)]
pub struct TestAppRequest {
    pub name: String,
    pub input: TriggerData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TestAppResponse {
    pub output: serde_json::Value,
}

// Not actually _used_ in http right now, just in CLI and in WAVS itself
// likely to be refactored as we finalize the "upload component" story
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ComponentSource {
    /// The wasm bytecode is provided directly.
    Bytecode(Vec<u8>),
    /// The wasm bytecode provided at fixed url, digest provided to ensure no tampering
    Download { url: String, digest: Digest },
    /// The wasm bytecode downloaded from a standard registry, digest provided to ensure no tampering
    Registry {
        // TODO: what info do we need here?
        // TODO: can we support some login info for private registries, as env vars in config or something?
        registry: String,
        digest: Digest,
    },
    /// An already deployed component
    Digest(Digest),
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
