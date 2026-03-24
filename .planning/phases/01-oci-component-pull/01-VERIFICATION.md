---
phase: 01-oci-component-pull
verified: 2026-03-24T21:12:54Z
status: human_needed
score: 5/5 must-haves verified
re_verification: false
human_verification:
  - test: "Deploy a service.json with an oci:// component URI against a live registry"
    expected: "Service deploys without requiring a local .wasm file; component is pulled, verified, and cached"
    why_human: "Requires a live OCI registry (e.g. ghcr.io) and a running WAVS node; cannot verify pull success programmatically"
  - test: "Deploy a service with a declared @sha256: digest that does not match the pulled component"
    expected: "WAVS refuses to deploy with a 'Component digest mismatch: expected X, got Y' error"
    why_human: "Requires a live registry and a mismatched digest; mismatch error path verified in code but not exercisable without a running node"
  - test: "Deploy the same OCI service twice consecutively and observe logs"
    expected: "Second deploy emits a cache-hit path (store_component_from_source returns early) with no re-pull from registry"
    why_human: "Cache behavior requires running node with disk storage; log inspection confirms OCI-04"
  - test: "Deploy with a tag-only oci:// URI (no @sha256: suffix)"
    expected: "WAVS emits a tracing::warn log containing 'without digest pin' before proceeding"
    why_human: "Warning exists in code but verifying it appears in actual runtime logs requires a live deployment"
  - test: "Deploy an OCI service with WAVS_OCI_USERNAME and WAVS_OCI_PASSWORD set to valid private registry credentials"
    expected: "Component is pulled successfully from private registry using Basic auth"
    why_human: "Requires a real private OCI registry with known credentials; auth code path verified in source but network integration cannot be checked statically"
---

# Phase 1: OCI Component Pull Verification Report

**Phase Goal:** Developers can deploy WAVS services that reference OCI-hosted WASM components by URI, with digest-verified pull and content-addressed caching
**Verified:** 2026-03-24T21:12:54Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

All five automated checks pass. The implementation is complete and substantive. Five human integration tests remain to confirm end-to-end runtime behavior.

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A `service.json` with `oci://` component URI deploys without requiring a local `.wasm` file | ? HUMAN | `ComponentSource::Oci` variant exists in service.rs with correct serde; OCI pull path wired in base_engine.rs; runtime confirmation needed |
| 2 | WAVS refuses to deploy a service whose pulled component does not match declared `@sha256:` digest | ? HUMAN | `Component digest mismatch: expected {}, got {}` error path verified in base_engine.rs:162-166; live test needed |
| 3 | Deploying the same service twice does not re-pull (cache hit in logs) | ? HUMAN | `store_component_from_source` checks `data_exists` before delegating to `load_component_from_source`; confirmed at wasm_engine.rs:75-78; log observation needed |
| 4 | Tag-only OCI URI emits a visible warning before proceeding | ? HUMAN | `tracing::warn!` with "without digest pin" exists at base_engine.rs:139-145; live log confirmation needed |
| 5 | Pulling from a private registry succeeds when credentials are set via env vars | ? HUMAN | `auth_from_env()` reads `WAVS_OCI_USERNAME`/`WAVS_OCI_PASSWORD` and returns `RegistryAuth::Basic`; unit test passes; live private registry test needed |

**Score:** 5/5 truths have complete, substantive, correctly-wired implementations. All 5 require human integration testing for runtime confirmation.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `packages/types/src/service.rs` | `ComponentSource::Oci` variant with `uri: String` and `digest: Option<ComponentDigest>` | VERIFIED | Lines 230-237: `Oci { uri: String, digest: Option<ComponentDigest> }` with `#[serde(default, skip_serializing_if = "Option::is_none")]` |
| `packages/utils/src/oci.rs` | `OciUri` parser and `OciPuller` with `auth_from_env()` | VERIFIED | Full implementation: `OciUri::parse()` at line 36, `OciPuller::pull()` at line 98, `auth_from_env()` at line 131; 202 lines non-empty |
| `packages/utils/src/lib.rs` | `pub mod oci` re-export | VERIFIED | Line 12: `pub mod oci;` present |
| `packages/engine/src/common/base_engine.rs` | `ComponentSource::Oci` arm in `load_component_from_source`; tuple return `(WasmComponent, ComponentDigest)` | VERIFIED | Lines 107-184: full Oci match arm with OciPuller call, digest verification, cache storage, unpinned-tag warning; return type is `Result<(WasmComponent, ComponentDigest), EngineError>` |
| `packages/wavs/src/subsystems/engine/wasm_engine.rs` | `store_component_from_source` with Option-aware digest handling | VERIFIED | Lines 70-96: `ComponentSource::Oci` arm included; cache check uses `source.digest()` returning `Option` |
| `Cargo.toml` (workspace) | `oci-client = "0.16"` and `oci-wasm = "0.4"` | VERIFIED | Lines 170-171 confirmed |
| `packages/utils/Cargo.toml` | `oci-client` and `oci-wasm` workspace deps | VERIFIED | Lines 18-19 confirmed |
| `packages/engine/src/bindings/types/wavs_to_component.rs` | `ComponentSource::Oci` arms in WIT type conversions | VERIFIED | Lines 233 and 791: both operator and aggregator world conversions handle `Oci` variant (maps to Download representation) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `packages/engine/src/common/base_engine.rs` | `packages/utils/src/oci.rs` | `OciPuller::pull()` call in Oci match arm | WIRED | `use utils::oci::{OciPuller, OciUri}` at line 131; `OciPuller::auth_from_env()` at line 147, `OciPuller::new()` at line 148, `puller.pull(&oci_uri, &auth)` at line 149 |
| `packages/engine/src/common/base_engine.rs` | `packages/types/src/service.rs` | `ComponentSource::Oci { uri, digest }` pattern match | WIRED | Match arm at line 130 destructures `Oci { uri, digest }` correctly |
| `packages/wavs/src/subsystems/engine/wasm_engine.rs` | `packages/engine/src/common/base_engine.rs` | `store_component_from_source` calls `load_component_from_source`, destructures `(WasmComponent, ComponentDigest)` tuple | WIRED | Line 85: `let (_component, digest) = self.engine.load_component_from_source(source).await?` |
| `packages/utils/src/oci.rs` | `oci-client` crate | `oci_client::Reference` parse and `WasmClient::pull()` | WIRED | `use oci_client::{client::ClientConfig, secrets::RegistryAuth, Client as OciClient, Reference}` at line 8; `use oci_wasm::WasmClient` at line 9 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| OCI-01 | 01-01-PLAN.md, 01-02-PLAN.md | `service.json` accepts `oci://` URIs as component source | SATISFIED | `ComponentSource::Oci { uri: String, .. }` in service.rs; `#[serde(rename_all = "snake_case")]` produces `"oci"` JSON key |
| OCI-02 | 01-01-PLAN.md, 01-02-PLAN.md | Components pulled from OCI registries at service deploy time | SATISFIED | `OciPuller::pull()` called in `load_component_from_source` which runs at deploy time via `store_component_from_source` in engine.rs:165,170 |
| OCI-03 | 01-02-PLAN.md | Pulled components verified by SHA256 digest before loading | SATISFIED | base_engine.rs:159-167: `ComponentDigest::hash(&bytes)` compared to `source.digest()`, returns `EngineError::StorageError("Component digest mismatch...")` on failure |
| OCI-04 | 01-02-PLAN.md | Pulled components cached on disk by digest (no re-pull for identical content) | SATISFIED | wasm_engine.rs:75-78: `data_exists` check before pull; base_engine.rs:169-172: `storage.set_data(&bytes)` stores by content hash; base_engine.rs:112-115: LRU + disk cache hit returns early |
| OCI-05 | 01-01-PLAN.md, 01-02-PLAN.md | Digest pinning (`@sha256:`) supported; deploy warns if only tag specified | SATISFIED | base_engine.rs:138-145: `tracing::warn!` fires when `oci_uri.is_unpinned() && digest.is_none()`; message: "Deploying OCI component without digest pin (@sha256:)" |
| OCI-06 | 01-01-PLAN.md, 01-02-PLAN.md | Authenticated pull via environment credentials for private registries | SATISFIED | oci.rs:131-145: `auth_from_env()` reads `WAVS_OCI_USERNAME` + `WAVS_OCI_PASSWORD`, returns `RegistryAuth::Basic(user, pass)` or falls back to `RegistryAuth::Anonymous` |

All 6 OCI requirements are SATISFIED by the implementation.

**Note on REQUIREMENTS.md status field:** REQUIREMENTS.md shows all OCI requirements as `Pending` with checkboxes unchecked. The traceability table shows `Status: Pending` for all six OCI requirements. This is a documentation gap — the requirements were fulfilled by the phase but the REQUIREMENTS.md file was not updated to reflect completion. This does not affect goal achievement, but should be corrected as a documentation task.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `packages/engine/src/common/base_engine.rs` | 199 | `// TODO: write precompiled wasm` | Info | Pre-existing TODO unrelated to OCI; no impact on phase goal |
| `packages/types/src/service.rs` | 270, 502, 633, 695 | Various `TODO`/`FIXME` naming comments | Info | All pre-existing notes unrelated to OCI; no impact |
| `packages/wavs/src/subsystems/engine/wasm_engine.rs` | 98 | `// TODO: paginate this` | Info | Pre-existing TODO unrelated to OCI; no impact |

No blockers or OCI-specific stubs found. All TODOs are pre-existing and unrelated to this phase.

### Build Verification

| Check | Result |
|-------|--------|
| `cargo check -p wavs-types` | Finished (0 errors) |
| `cargo check -p utils` | Finished (0 errors) |
| `cargo check -p wavs-engine` | Finished (0 errors) |
| `cargo check` (full workspace) | Finished (0 errors, all crates) |
| `cargo test -p utils -- oci::tests` | 5 passed, 0 failed |
| `cargo test -p wavs-types` | All tests passed |

### Human Verification Required

#### 1. End-to-end OCI deploy

**Test:** Write a `service.json` with `"source": {"oci": {"uri": "oci://ghcr.io/layerlabs/echo-data:latest"}}`, deploy via `wavs` CLI or dev-tool against a running WAVS node.
**Expected:** Service deploys successfully; WAVS logs show "Pulling WASM component from OCI registry" and "OCI pull complete"; no `.wasm` file required locally.
**Why human:** Requires a live OCI registry, running WAVS node, and inspecting runtime logs.

#### 2. Digest mismatch rejection (OCI-03)

**Test:** Deploy a service with an `oci://` URI and a deliberately wrong `digest` field (valid hex SHA256, but not matching the actual component).
**Expected:** Deploy fails with error containing "Component digest mismatch: expected X, got Y".
**Why human:** Requires a live registry and the ability to observe the error response from the WAVS deploy endpoint.

#### 3. Cache hit on second deploy (OCI-04)

**Test:** Deploy the same OCI service twice in succession. Observe WAVS logs for the second deploy.
**Expected:** Second deploy returns immediately from `store_component_from_source` cache check (`data_exists` returns true); no "Pulling WASM component from OCI registry" log on the second call.
**Why human:** Cache behavior requires disk-persisted storage and log observation from a running node.

#### 4. Unpinned tag warning (OCI-05)

**Test:** Deploy a service with a tag-only OCI URI (e.g., `oci://ghcr.io/org/component:latest` with no `@sha256:` and no `digest` field).
**Expected:** WAVS logs contain a `WARN` level entry with "Deploying OCI component without digest pin (@sha256:). The component content may change if the tag is updated."
**Why human:** Requires running node and log level set to include WARN output.

#### 5. Private registry authentication (OCI-06)

**Test:** Set `WAVS_OCI_USERNAME` and `WAVS_OCI_PASSWORD` to valid credentials for a private OCI registry, then deploy a service referencing a private component image.
**Expected:** Component is pulled successfully using Basic auth; no authentication error from the registry.
**Why human:** Requires a real private OCI registry with test credentials; the `auth_from_env()` code path is verified but network integration cannot be confirmed statically.

### Summary

All six OCI requirements (OCI-01 through OCI-06) have complete, substantive, and correctly-wired implementations:

- `ComponentSource::Oci` deserializes from `service.json` with the `oci` serde key (OCI-01)
- `OciPuller::pull()` is invoked at deploy time via `load_component_from_source` (OCI-02)
- `ComponentDigest::hash()` is compared to the declared digest before caching (OCI-03)
- `CAStorage::data_exists()` is checked before pulling to provide cache hits (OCI-04)
- `tracing::warn!` fires on unpinned-tag deployments (OCI-05)
- `auth_from_env()` reads `WAVS_OCI_USERNAME`/`WAVS_OCI_PASSWORD` with Anonymous fallback (OCI-06)

The full workspace compiles cleanly. All 5 OCI URI parsing unit tests pass. The only remaining gaps are 5 integration tests that require a live WAVS node and OCI registry — these are inherently human-testable and cannot be verified statically.

One documentation gap noted: `REQUIREMENTS.md` still shows all OCI requirements as `Pending` with unchecked checkboxes. This should be updated to reflect completion.

---

_Verified: 2026-03-24T21:12:54Z_
_Verifier: Claude (gsd-verifier)_
