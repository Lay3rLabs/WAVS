# Phase 1: OCI Component Pull - Research

**Researched:** 2026-03-24
**Domain:** OCI registry integration for WASM component distribution in Rust
**Confidence:** HIGH

## Summary

WAVS already has a content-addressable storage system (`CAStorage` trait backed by `FileStorage`) and two existing `ComponentSource` variants for remote component acquisition: `Download` (HTTP/IPFS URI + digest) and `Registry` (wasm-pkg-client/Warg namespace routing). Phase 1 adds a third variant -- `Oci` -- that pulls WASM components directly from OCI-compliant registries (ghcr.io, Docker Hub, private registries) using the standard `oci://` URI scheme.

The `oci-client` (v0.15.0) and `oci-wasm` (v0.3.0) crates are already in `Cargo.lock` as transitive dependencies of `wasm-pkg-client`. The implementation should add `oci-client` (v0.16.1) and `oci-wasm` (v0.4.0) as direct workspace dependencies and create an OCI pull module alongside the existing `WkgClient` in `packages/utils/src/`. The core integration point is `BaseEngine::load_component_from_source()` in `packages/engine/src/common/base_engine.rs`, which already has a pattern-match on `ComponentSource` variants that handles download, digest verification, and storage. Adding an `Oci` arm follows the identical pattern.

**Primary recommendation:** Add `ComponentSource::Oci { uri: String, digest: Option<ComponentDigest> }` to `packages/types/src/service.rs`, implement an `OciPuller` module in `packages/utils/src/oci.rs`, and wire it into the existing `load_component_from_source` match in `base_engine.rs`. The digest field is `Option` to support tag-only references (with a warning), while pinned `@sha256:` references populate the digest field for mandatory verification.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OCI-01 | `service.json` accepts `oci://` URIs as component source | New `ComponentSource::Oci` variant in `packages/types/src/service.rs`; serde deserialization handles `oci://` prefix |
| OCI-02 | Components are pulled from OCI registries at service deploy time | `OciPuller` module using `oci-wasm::WasmClient::pull()` called from `BaseEngine::load_component_from_source()` |
| OCI-03 | Pulled components are verified by SHA256 digest before loading | Existing `ComponentDigest::hash(&bytes)` comparison in `base_engine.rs` pattern; fail-fast on mismatch |
| OCI-04 | Pulled components are cached on disk by digest (no re-pull for identical content) | Existing `CAStorage::data_exists()` check at top of `store_component_from_source()` already skips fetch for known digests |
| OCI-05 | Digest pinning (`@sha256:`) is supported; deploy warns if only tag is specified | URI parsing extracts `@sha256:` suffix; if absent, `tracing::warn!` before proceeding |
| OCI-06 | Authenticated pull supported via environment credentials for private registries | `oci_client::RegistryAuth::Basic(username, password)` from env vars `WAVS_OCI_USERNAME` / `WAVS_OCI_PASSWORD` |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `oci-client` | 0.16.1 | OCI Distribution spec client (pull manifests, blobs) | ORAS project; the standard Rust OCI client. Wassette uses 0.16. Provides `Client`, `Reference`, `RegistryAuth`. |
| `oci-wasm` | 0.4.0 | WASM-specific OCI artifact wrapper | Bytecode Alliance crate. Wraps `oci-client` with correct WASM media types (`application/wasm`, `application/vnd.wasm.config.v0+json`). Provides `WasmClient::pull()`. |

### Supporting (already in workspace)

| Library | Version | Role in This Phase |
|---------|---------|-------------------|
| `sha2` | 0.10.9 | SHA256 digest computation for pulled components (via existing `ComponentDigest::hash()`) |
| `const-hex` | 1.16.0 | Hex encoding/decoding for digest strings |
| `tracing` | 0.1.41 | Logging warnings for unpinned tags, pull progress |
| `reqwest` | 0.12.23 | Transitive dependency for `oci-client` HTTP transport |
| `tokio` | 1.47.1 | Async runtime for pull operations |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `oci-client` + `oci-wasm` direct | Extend `wasm-pkg-client` (`WkgClient`) | `wasm-pkg-client` routes by Warg package namespace, not raw OCI URIs. The `oci://ghcr.io/user/component:tag` format bypasses namespace resolution entirely. Direct OCI client is simpler and matches Wassette's approach. |
| `oci-wasm::WasmClient::pull()` | Raw `oci-client::Client::pull()` with manual media type filtering | `oci-wasm` adds ~200 lines of media type handling. Without it, we'd need to manually filter layers by `application/wasm` media type and handle the WASM-specific manifest config. Not worth hand-rolling. |
| Environment variable auth | Docker credential helper integration (`docker_credential` crate) | Docker credential helpers add complexity. Env var auth (`WAVS_OCI_USERNAME` / `WAVS_OCI_PASSWORD`) is the standard for CI/CD and operator deployments. `docker_credential` is already a transitive dep through `wasm-pkg-client` if needed later. |

**Installation (workspace Cargo.toml):**

```toml
# Add to [workspace.dependencies]
oci-client = "0.16"
oci-wasm = "0.4"
```

**Version verification:** `oci-client` 0.16.1 is the latest on crates.io as of 2026-03-24. `oci-wasm` 0.4.0 is the latest. `oci-wasm 0.4.0` declares `oci-client = "0.16"` in its Cargo.toml, so they are compatible. The existing `Cargo.lock` has `oci-client 0.15.0` and `oci-wasm 0.3.0` as transitive deps of `wasm-pkg-client 0.12.0` -- Cargo will resolve both the old (transitive) and new (direct) versions, which is acceptable since they are different semver-incompatible versions.

## Architecture Patterns

### Where the New Code Goes

```
packages/
  types/src/service.rs           # Add ComponentSource::Oci variant
  utils/src/oci.rs               # NEW: OciPuller module (parse URI, auth, pull)
  utils/src/lib.rs               # Re-export oci module
  engine/src/common/base_engine.rs  # Add Oci arm to load_component_from_source()
  engine/Cargo.toml              # Add oci-client, oci-wasm deps
  utils/Cargo.toml               # Add oci-client, oci-wasm deps
```

### Pattern 1: ComponentSource::Oci Variant

**What:** A new enum variant in the existing `ComponentSource` enum that captures an OCI URI and optional digest.

**When to use:** Any service definition that references a WASM component hosted on an OCI registry.

**Example:**

```rust
// In packages/types/src/service.rs
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComponentSource {
    Download { uri: UriString, digest: ComponentDigest },
    Registry { #[serde(flatten)] registry: Registry },
    #[cfg_attr(feature = "ts-bindings", ts(type = "string"))]
    Digest(ComponentDigest),
    // NEW:
    Oci {
        /// Full OCI URI, e.g. "oci://ghcr.io/org/component:v1.0@sha256:abc123..."
        uri: String,
        /// Digest for verification. Populated from @sha256: suffix in URI.
        /// If None, pull resolves the tag to a digest and warns about unpinned references.
        digest: Option<ComponentDigest>,
    },
}
```

**Critical design choice:** The `digest` field is `Option<ComponentDigest>` rather than mandatory. This is required by OCI-05 -- tag-only references (`:latest`, `:v1.0`) must be deployable with a warning. When `digest` is `Some`, verification is mandatory (OCI-03). When `None`, the puller resolves the tag, computes the digest from the pulled content, and logs a warning.

The `ComponentSource::digest()` method needs updating:

```rust
impl ComponentSource {
    pub fn digest(&self) -> Option<&ComponentDigest> {
        match self {
            ComponentSource::Download { digest, .. } => Some(digest),
            ComponentSource::Registry { registry } => Some(&registry.digest),
            ComponentSource::Digest(digest) => Some(digest),
            ComponentSource::Oci { digest, .. } => digest.as_ref(),
        }
    }
}
```

Note: The current `digest()` returns `&ComponentDigest` (not `Option`). This is a breaking change. The alternative is to keep `digest` mandatory and parse the `@sha256:` at deserialization time, storing the tag-resolved digest after pull. The recommended approach: parse `@sha256:` at construction/deserialization into the `digest` field. For tag-only URIs, leave `digest` as `None` and resolve after pull. This requires changing `digest()` to return `Option`, or adding a separate method. The planner should decide between:

- **Option A:** Change `digest()` to `Option<&ComponentDigest>` (breaks callers, but cleanest)
- **Option B:** Keep `digest` mandatory, resolve tag to digest during a pre-pull "resolve" step before `store_component_from_source` is called
- **Option C:** Store a sentinel/zero digest for tag-only refs and compute on pull

**Recommendation:** Option A is cleanest. The existing callers of `digest()` are in `store_component_from_source` (line 74 of wasm_engine.rs) and `base_engine.rs` (line 112, 134). Both can handle `Option` with minor changes. The `Digest` variant's use as "component already uploaded locally" means it always has a digest, which aligns.

### Pattern 2: OCI URI Parsing

**What:** Parse `oci://ghcr.io/org/component:tag@sha256:hexdigest` into an `oci_client::Reference` and optional digest.

**When to use:** At the boundary between service.json deserialization and the OCI pull operation.

**Example:**

```rust
// In packages/utils/src/oci.rs
use oci_client::Reference;

pub struct OciUri {
    pub reference: Reference,
    pub digest: Option<String>,  // sha256:hex...
}

impl OciUri {
    pub fn parse(uri: &str) -> anyhow::Result<Self> {
        // Strip oci:// prefix
        let raw = uri.strip_prefix("oci://")
            .ok_or_else(|| anyhow::anyhow!("OCI URI must start with oci://"))?;

        // oci-client's Reference::from_str handles:
        //   ghcr.io/org/component:tag
        //   ghcr.io/org/component@sha256:abc123
        //   ghcr.io/org/component:tag@sha256:abc123
        let reference: Reference = raw.parse()?;

        let digest = reference.digest().map(|d| d.to_string());

        Ok(OciUri { reference, digest })
    }
}
```

### Pattern 3: OCI Pull with Auth

**What:** Pull a WASM component from an OCI registry using the standard `oci-wasm` `WasmClient`.

**When to use:** Called from `load_component_from_source` when `ComponentSource::Oci` is matched.

**Example:**

```rust
use oci_client::{Client, secrets::RegistryAuth, client::ClientConfig};
use oci_wasm::WasmClient;

pub struct OciPuller {
    client: WasmClient,
}

impl OciPuller {
    pub fn new() -> Self {
        let config = ClientConfig::default();
        let oci_client = Client::new(config);
        Self {
            client: WasmClient::new(oci_client),
        }
    }

    pub async fn pull(
        &self,
        uri: &OciUri,
        auth: &RegistryAuth,
    ) -> anyhow::Result<Vec<u8>> {
        let image_data = self.client.pull(&uri.reference, auth).await?;

        // oci-wasm pull returns ImageData with layers
        // The WASM binary is the first (and typically only) layer with
        // media type application/wasm
        let wasm_layer = image_data.layers
            .into_iter()
            .find(|l| l.media_type == oci_wasm::WASM_LAYER_MEDIA_TYPE)
            .ok_or_else(|| anyhow::anyhow!("No WASM layer found in OCI manifest"))?;

        Ok(wasm_layer.data)
    }

    pub fn auth_from_env() -> RegistryAuth {
        match (
            std::env::var("WAVS_OCI_USERNAME"),
            std::env::var("WAVS_OCI_PASSWORD"),
        ) {
            (Ok(user), Ok(pass)) => RegistryAuth::Basic(user, pass),
            _ => RegistryAuth::Anonymous,
        }
    }
}
```

### Pattern 4: Integration with BaseEngine

**What:** Add the `Oci` arm to the existing `load_component_from_source` match in `base_engine.rs`.

**When to use:** This is the core integration point where OCI pulls are triggered.

```rust
// In packages/engine/src/common/base_engine.rs :: load_component_from_source()
ComponentSource::Oci { uri, digest } => {
    let oci_uri = OciUri::parse(uri)?;
    let auth = OciPuller::auth_from_env();

    // Warn if no digest pinning
    if oci_uri.digest.is_none() && digest.is_none() {
        tracing::warn!(
            uri = %uri,
            "Deploying OCI component without digest pin (@sha256:). \
             The component content may change if the tag is updated. \
             Pin with @sha256:<digest> for reproducible deploys."
        );
    }

    let puller = OciPuller::new();
    let bytes = puller.pull(&oci_uri, &auth).await?;

    // Verify digest if provided
    let computed_digest = ComponentDigest::hash(&bytes);
    if let Some(expected) = digest {
        if computed_digest != *expected {
            return Err(EngineError::StorageError(
                format!("OCI component digest mismatch: expected {}, got {}",
                    expected, computed_digest)
            ));
        }
    }

    bytes
}
```

### Anti-Patterns to Avoid

- **Pulling at node boot time (not deploy time):** The REQUIREMENTS.md explicitly says "Pull at deploy time, not boot time." The pull happens in `store_components_for_service` which is called from `add_service_direct`. Do not add OCI pull logic to the node startup sequence.
- **Creating a separate cache for OCI components:** The existing `CAStorage` (FileStorage) already provides content-addressed caching by digest. OCI-pulled components go through the same `storage.set_data(&bytes)` call as Download and Registry variants. The `data_exists` check at the top of `store_component_from_source` prevents re-pulls.
- **Requiring digest for all OCI references:** OCI-05 explicitly requires tag-only references to work (with a warning). Making digest mandatory would break `:latest` tag usage.
- **Caching the `WasmClient` / OCI client at the engine level:** The `WasmClient` wraps an `oci_client::Client` which manages its own internal HTTP client and auth token cache. Creating one per pull is fine for v1. If performance becomes an issue (many concurrent pulls), the client can be cached later.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OCI manifest parsing | Custom manifest JSON parser | `oci-client::Client::pull()` returns structured `ImageData` | OCI manifests have multiple formats (v1, v2, OCI Image Index); the client handles all of them |
| WASM media type detection | String comparison on layer types | `oci-wasm::WasmClient::pull()` filters to WASM layers | `oci-wasm` knows the exact media types (`application/wasm`, `application/vnd.wasm.config.v0+json`) and errors if no WASM layer is found |
| OCI reference parsing | Regex-based URI parser | `oci_client::Reference::from_str()` | Handles registry defaults (docker.io), port numbers, digest/tag combinations, validation |
| Content-addressed storage | New OCI-specific cache directory | Existing `CAStorage::set_data()` / `data_exists()` | Already provides digest-based deduplication, directory sharding, and the exact behavior OCI-04 requires |
| Docker credential management | Environment variable + config file parser | `oci_client::RegistryAuth` enum (Anonymous/Basic/Bearer) | Clean abstraction; env vars for v1, `docker_credential` crate available as transitive dep for v2 |

**Key insight:** The WAVS codebase already has 90% of the infrastructure needed. The engine's `load_component_from_source` pattern (fetch, verify digest, store in CA storage, cache in LRU) is exactly what OCI pull needs. The new code is primarily: URI parsing, `oci-wasm` client invocation, and the `ComponentSource::Oci` type definition.

## Common Pitfalls

### Pitfall 1: OCI Client Version Conflict with wasm-pkg-client

**What goes wrong:** `wasm-pkg-client 0.12.0` depends on `oci-client 0.15.0` and `oci-wasm 0.3.0`. Adding direct deps on `oci-client 0.16.1` and `oci-wasm 0.4.0` causes Cargo to compile both versions (they are semver-incompatible). This is technically fine (Cargo handles it), but types from `oci-client 0.15` are incompatible with `oci-client 0.16`.

**Why it happens:** The OCI module in `packages/utils/src/oci.rs` and the WKG module in `packages/utils/src/wkg.rs` both live in the same crate. If they try to share types across the two oci-client versions, compilation fails.

**How to avoid:** Keep the OCI puller module fully self-contained. It uses `oci-client 0.16` types internally and exposes only `Vec<u8>` (raw bytes) to the rest of the codebase. The `WkgClient` continues using its own `oci-client 0.15` transitively through `wasm-pkg-client`. They never share types across versions.

**Warning signs:** Compiler errors about "expected `oci_client::Reference` but found `oci_client::Reference`" (same type name, different versions).

### Pitfall 2: Digest Format Mismatch (OCI vs WAVS)

**What goes wrong:** OCI digests use the format `sha256:abcdef...` (with `sha256:` prefix). WAVS `ComponentDigest` stores raw 64-char hex (no prefix). If the digest from the OCI manifest is compared directly with the `ComponentDigest`, it will never match.

**Why it happens:** Different conventions for the same underlying data.

**How to avoid:** When extracting a digest from an OCI URI's `@sha256:...` suffix, strip the `sha256:` prefix before converting to `ComponentDigest`. When computing the digest of pulled bytes, use `ComponentDigest::hash(&bytes)` which produces the raw hex format. The OCI manifest digest (which covers the compressed layer, not the raw content) is NOT the same as the WAVS component digest (which covers the raw WASM bytes). Always recompute from raw bytes.

**Warning signs:** Digests that look correct but have a `sha256:` prefix, or digests that match the OCI manifest layer digest but not the content digest.

### Pitfall 3: Misunderstanding What the OCI Digest Pins

**What goes wrong:** The `@sha256:` in an OCI reference (e.g., `ghcr.io/org/component@sha256:abc`) is the **manifest** digest, not the content digest. The manifest digest identifies which manifest to pull (and thus which layers). The actual WASM bytes have their own content digest. The WAVS `ComponentDigest` is a SHA256 of the raw WASM bytes.

**Why it happens:** OCI has multiple digest layers: manifest digest, config digest, and layer (content) digest.

**How to avoid:** Use the `@sha256:` from the URI to ensure the right manifest is pulled (oci-client handles this automatically). After pulling, compute `ComponentDigest::hash(&wasm_bytes)` from the actual content and compare against the `service.json` digest field. The URI's `@sha256:` is for registry-level immutability; the service.json `digest` field is for WAVS-level content verification.

### Pitfall 4: service.json Backward Compatibility

**What goes wrong:** Adding a new `ComponentSource::Oci` variant could break existing service.json files if serde deserialization is not handled carefully.

**Why it happens:** The existing `ComponentSource` enum uses `#[serde(rename_all = "snake_case")]` which means variants are serialized as `"download"`, `"registry"`, `"digest"`. The new `"oci"` variant must follow the same convention.

**How to avoid:** The serde tag for the new variant is simply `"oci"` (snake_case of `Oci`). Existing service.json files using `"download"`, `"registry"`, or `"digest"` continue to work unchanged. Test with existing test service definitions to confirm backward compatibility.

**Warning signs:** Deserialization errors on existing test fixtures after adding the variant.

### Pitfall 5: Blocking the Tokio Runtime with Synchronous OCI Client Internals

**What goes wrong:** `oci-client` uses `reqwest` internally, which is async. But if any synchronous filesystem operations or DNS resolution blocks the async runtime, pull operations could stall other services.

**Why it happens:** The pull operation involves network I/O and potentially large downloads. If called on a constrained tokio worker thread, it could block other tasks.

**How to avoid:** The existing pattern in `base_engine.rs` is already called from async context (`load_component_from_source` is `async fn`). The `oci-wasm::WasmClient::pull()` is async and uses `reqwest` internally (same as existing `fetch_bytes`). No special handling needed beyond what exists.

## Code Examples

### service.json with OCI Source (Digest-Pinned)

```json
{
  "name": "my-oci-service",
  "status": "active",
  "manager": {
    "evm": {
      "chain": "evm:31337",
      "address": "0xAbCd1234..."
    }
  },
  "workflows": {
    "default": {
      "trigger": "manual",
      "component": {
        "source": {
          "oci": {
            "uri": "oci://ghcr.io/layerlabs/echo-data:v1.0",
            "digest": "f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420"
          }
        },
        "permissions": {},
        "fuel_limit": null,
        "time_limit_seconds": null,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  }
}
```

### service.json with OCI Source (Tag-Only, Will Warn)

```json
{
  "source": {
    "oci": {
      "uri": "oci://ghcr.io/layerlabs/echo-data:latest"
    }
  }
}
```

### OCI URI with Inline Digest Pin

The `@sha256:` suffix in the URI provides registry-level immutability (the manifest you pull is content-addressed), while the `digest` field in the JSON provides WAVS-level content verification. Both can be present. If only the URI has `@sha256:`, the pull is deterministic but WAVS does not verify the content hash (unless the `digest` JSON field is also set). The recommended UX: if the user provides `@sha256:` in the URI but no `digest` field, compute the digest after pull and store it (log for the user).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `oci-distribution` crate | `oci-client` (renamed) | 2024 | Same crate, new name. ORAS project renamed it. `oci-distribution` is deprecated. |
| `oci-wasm 0.3` | `oci-wasm 0.4` | 2025 | Updated to `oci-client 0.16`. WASM-specific types unchanged. |
| Docker v2 manifest only | OCI Image Manifest + OCI Image Index | 2023+ | Modern registries use OCI manifest format. `oci-client` handles both. |
| WASM media type `application/vnd.module.wasm.content.layer.v1+wasm` | `application/wasm` | 2024 | CNCF standardized the media type. `oci-wasm` uses the correct current value. |

**Deprecated/outdated:**
- `oci-distribution` crate: Renamed to `oci-client`. Do not use the old name.
- `application/vnd.module.wasm.content.layer.v1+wasm`: Replaced by `application/wasm`.
- `wasm-pkg-client` for raw OCI URIs: Not designed for this; use `oci-client` + `oci-wasm` directly.

## Open Questions

1. **`ComponentSource::digest()` return type change**
   - What we know: Current signature is `fn digest(&self) -> &ComponentDigest`. The `Oci` variant with tag-only references has no digest until after pull.
   - What's unclear: Whether to change the return type to `Option<&ComponentDigest>` (cleaner but breaks callers) or use a pre-pull resolve step to always populate the digest.
   - Recommendation: Change to `Option<&ComponentDigest>`. There are only ~5 call sites. The planner should assess the blast radius and include fixing all callers as tasks.

2. **Environment variable naming for OCI auth**
   - What we know: The project uses `WAVS_` prefix for env vars. OCI auth needs username + password (for `RegistryAuth::Basic`).
   - What's unclear: Whether to use `WAVS_OCI_USERNAME` / `WAVS_OCI_PASSWORD` or a single `WAVS_OCI_AUTH` with `username:password` format, or integrate with Docker credential helpers.
   - Recommendation: `WAVS_OCI_USERNAME` / `WAVS_OCI_PASSWORD` for v1. Simple, standard, works in CI/CD. Add Docker credential helper support in v2.

3. **Per-registry auth vs global auth**
   - What we know: An operator might pull from multiple registries (ghcr.io for public, private-registry.company.com for enterprise).
   - What's unclear: Whether the env vars should be global or per-registry.
   - Recommendation: Global env vars for v1 (same credentials used for all registries). Per-registry auth can be added later through a TOML config section similar to the existing `wasm-pkg-client` config format.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `packages/types/src/service.rs` -- `ComponentSource` enum, `ComponentDigest` type, `Registry` struct
- Existing codebase: `packages/engine/src/common/base_engine.rs` -- `load_component_from_source()`, digest verification pattern, CA storage integration
- Existing codebase: `packages/utils/src/wkg.rs` -- `WkgClient` pattern for registry pulls, `wasm-pkg-client` usage
- Existing codebase: `packages/wavs/src/dispatcher.rs` -- `add_service_direct()`, `store_components_for_service()` integration point
- Existing codebase: `packages/utils/src/storage/fs.rs` -- `FileStorage` content-addressed storage implementation
- [oci-client 0.16.1 on crates.io](https://crates.io/crates/oci-client) -- verified latest version
- [oci-wasm 0.4.0 on crates.io](https://crates.io/crates/oci-wasm) -- verified latest version
- [oci-client docs.rs -- Client methods](https://docs.rs/oci-client/0.16.1/oci_client/struct.Client.html) -- pull, auth, Reference
- [oci-client docs.rs -- RegistryAuth](https://docs.rs/oci-client/0.16.1/oci_client/secrets/enum.RegistryAuth.html) -- Anonymous, Basic, Bearer variants confirmed
- [oci-wasm docs.rs -- WasmClient](https://docs.rs/oci-wasm/0.4.0/oci_wasm/struct.WasmClient.html) -- pull(), push() methods confirmed
- [oci-client GitHub (ORAS project)](https://github.com/oras-project/rust-oci-client) -- RegistryAuth source confirmed

### Secondary (MEDIUM confidence)
- `.planning/research/STACK.md` -- Prior project research on OCI crate selection (verified against crates.io)
- `.planning/research/FEATURES.md` -- Prior feature landscape analysis
- [CNCF TAG Runtime WASM OCI Artifact spec](https://tag-runtime.cncf.io/wgs/wasm/deliverables/wasm-oci-artifact/) -- media types and manifest format
- [Bytecode Alliance rust-oci-wasm GitHub](https://github.com/bytecodealliance/rust-oci-wasm) -- oci-wasm source

### Tertiary (LOW confidence)
- None -- all critical claims verified against primary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- versions verified on crates.io, API confirmed via docs.rs, existing transitive usage in Cargo.lock
- Architecture: HIGH -- integration points verified by reading existing source code; patterns follow established conventions in the codebase
- Pitfalls: HIGH -- identified from actual codebase analysis (version conflicts visible in Cargo.lock, digest format differences visible in type definitions)

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable domain; OCI spec and crate APIs change slowly)
