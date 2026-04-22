//! OCI registry client for pulling WASM components.
//!
//! Pulls WASM components from OCI-compliant registries (ghcr.io, Docker Hub, private registries)
//! using the `oci://` URI scheme. Components are returned as raw bytes for downstream
//! digest verification and content-addressed storage.

use anyhow::{anyhow, Result};
use oci_client::{client::ClientConfig, secrets::RegistryAuth, Client as OciClient, Reference};
use oci_wasm::WasmClient;

/// Parsed OCI URI components.
///
/// Splits an `oci://registry/repo:tag@sha256:digest` URI into an
/// `oci_client::Reference` (for the pull) and an optional digest string
/// (for WAVS-level content verification).
#[derive(Debug, Clone)]
pub struct OciUri {
    /// The OCI reference used by oci-client for the pull operation.
    pub reference: Reference,
    /// The `sha256:...` digest extracted from the URI's `@sha256:` suffix, if present.
    /// This is the OCI *manifest* digest, not the WASM content digest.
    /// When present, it ensures the registry returns the exact manifest requested.
    pub manifest_digest: Option<String>,
}

impl OciUri {
    /// Parse an `oci://` prefixed URI into its components.
    ///
    /// Accepts:
    /// - `oci://ghcr.io/org/component:tag`
    /// - `oci://ghcr.io/org/component@sha256:abc123...`
    /// - `oci://ghcr.io/org/component:tag@sha256:abc123...`
    ///
    /// Returns an error if the URI does not start with `oci://` or the reference
    /// portion is not a valid OCI reference.
    pub fn parse(uri: &str) -> Result<Self> {
        let raw = uri
            .strip_prefix("oci://")
            .ok_or_else(|| anyhow!("OCI URI must start with oci://, got: {}", uri))?;

        // oci_client::Reference::from_str handles:
        //   ghcr.io/org/component:tag
        //   ghcr.io/org/component@sha256:abc123
        //   ghcr.io/org/component:tag@sha256:abc123
        let reference: Reference = raw
            .parse()
            .map_err(|e| anyhow!("Invalid OCI reference '{}': {}", raw, e))?;

        let manifest_digest = reference.digest().map(|d| d.to_string());

        Ok(OciUri {
            reference,
            manifest_digest,
        })
    }

    /// Returns true if this URI has no `@sha256:` digest pin.
    /// Tag-only references resolve to whatever the registry currently maps the tag to,
    /// which may change over time.
    pub fn is_unpinned(&self) -> bool {
        self.manifest_digest.is_none()
    }
}

/// Pulls WASM components from OCI registries.
///
/// Wraps `oci-wasm::WasmClient` which handles WASM-specific OCI media types
/// (`application/wasm`, `application/vnd.wasm.config.v0+json`).
///
/// # Versioning note
/// This module uses `oci-client` 0.16 / `oci-wasm` 0.4 as direct dependencies.
/// The existing `wasm-pkg-client` depends on `oci-client` 0.15 transitively.
/// These are kept strictly separate -- this module exposes only `Vec<u8>` (raw bytes)
/// to avoid type conflicts between the two oci-client versions.
pub struct OciPuller {
    client: WasmClient,
}

impl OciPuller {
    /// Create a new OCI puller with default client configuration.
    pub fn new() -> Self {
        let config = ClientConfig::default();
        let oci_client = OciClient::new(config);
        Self {
            client: WasmClient::new(oci_client),
        }
    }

    /// Pull a WASM component from an OCI registry.
    ///
    /// Returns the raw WASM bytes. The caller is responsible for digest
    /// verification and storage.
    ///
    /// # Errors
    /// - Registry is unreachable or returns an error
    /// - The manifest contains no layer with WASM media type
    /// - Authentication fails for private registries
    pub async fn pull(&self, uri: &OciUri, auth: &RegistryAuth) -> Result<Vec<u8>> {
        tracing::info!(
            reference = %uri.reference,
            pinned = !uri.is_unpinned(),
            "Pulling WASM component from OCI registry"
        );

        let image_data = self
            .client
            .pull(&uri.reference, auth)
            .await
            .map_err(|e| anyhow!("OCI pull failed for {}: {}", uri.reference, e))?;

        // oci-wasm returns ImageData with layers filtered to WASM media types.
        // The WASM binary is the first (and typically only) layer.
        let wasm_layer =
            image_data.layers.into_iter().next().ok_or_else(|| {
                anyhow!("No WASM layer found in OCI manifest for {}", uri.reference)
            })?;

        tracing::info!(
            reference = %uri.reference,
            size_bytes = wasm_layer.data.len(),
            "OCI pull complete"
        );

        Ok(wasm_layer.data.to_vec())
    }

    /// Build `RegistryAuth` from environment variables.
    ///
    /// Reads `WAVS_OCI_USERNAME` and `WAVS_OCI_PASSWORD`. Both must be set
    /// for Basic auth; otherwise falls back to Anonymous.
    pub fn auth_from_env() -> RegistryAuth {
        match (
            std::env::var("WAVS_OCI_USERNAME"),
            std::env::var("WAVS_OCI_PASSWORD"),
        ) {
            (Ok(user), Ok(pass)) => {
                tracing::debug!("Using OCI Basic auth from WAVS_OCI_USERNAME/WAVS_OCI_PASSWORD");
                RegistryAuth::Basic(user, pass)
            }
            _ => {
                tracing::debug!("No OCI credentials found, using anonymous auth");
                RegistryAuth::Anonymous
            }
        }
    }
}

impl Default for OciPuller {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_oci_uri_with_tag() {
        let uri = OciUri::parse("oci://ghcr.io/layerlabs/echo-data:v1.0").unwrap();
        assert!(uri.is_unpinned());
        assert!(uri.manifest_digest.is_none());
        // Reference should contain the tag
        assert!(uri.reference.tag().is_some() || uri.reference.digest().is_none());
    }

    #[test]
    fn parse_oci_uri_with_digest() {
        let uri = OciUri::parse(
            "oci://ghcr.io/layerlabs/echo-data@sha256:abc123def456abc123def456abc123def456abc123def456abc123def456abcd"
        ).unwrap();
        assert!(!uri.is_unpinned());
        assert!(uri.manifest_digest.is_some());
        assert!(uri.manifest_digest.unwrap().starts_with("sha256:"));
    }

    #[test]
    fn parse_oci_uri_rejects_non_oci_prefix() {
        let result = OciUri::parse("https://ghcr.io/layerlabs/echo-data:v1.0");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("oci://"));
    }

    #[test]
    fn parse_oci_uri_with_tag_and_digest() {
        let uri = OciUri::parse(
            "oci://ghcr.io/layerlabs/echo-data:v1.0@sha256:abc123def456abc123def456abc123def456abc123def456abc123def456abcd"
        ).unwrap();
        assert!(!uri.is_unpinned());
        assert!(uri.manifest_digest.is_some());
    }

    #[test]
    fn auth_from_env_anonymous_when_no_vars() {
        // This test relies on WAVS_OCI_USERNAME not being set in the test environment
        // which is the default case
        let auth = OciPuller::auth_from_env();
        assert!(matches!(auth, RegistryAuth::Anonymous));
    }
}
