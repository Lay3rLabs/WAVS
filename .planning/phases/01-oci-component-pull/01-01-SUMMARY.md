---
phase: 01-oci-component-pull
plan: 01
subsystem: types, utils
tags: [oci, wasm, registry, oci-client, oci-wasm, component-source]

# Dependency graph
requires: []
provides:
  - "ComponentSource::Oci variant with uri: String and digest: Option<ComponentDigest>"
  - "OciUri parser for oci:// URIs with tag, digest, and tag+digest support"
  - "OciPuller client wrapping oci-wasm for authenticated WASM component pulls"
  - "OciPuller::auth_from_env() for WAVS_OCI_USERNAME/WAVS_OCI_PASSWORD credential loading"
  - "oci-client 0.16 and oci-wasm 0.4 workspace dependencies"
affects: [01-oci-component-pull plan 02, engine, cli]

# Tech tracking
tech-stack:
  added: [oci-client 0.16, oci-wasm 0.4]
  patterns: [OCI URI parsing with oci:// prefix, env-based registry auth, raw bytes return from pull]

key-files:
  created:
    - packages/utils/src/oci.rs
  modified:
    - Cargo.toml
    - packages/types/src/service.rs
    - packages/utils/src/lib.rs
    - packages/utils/Cargo.toml

key-decisions:
  - "digest() method returns Option<&ComponentDigest> to accommodate Oci variant where digest may be absent (tag-only pulls)"
  - "OciPuller exposes only Vec<u8> to avoid oci-client type version conflicts with wasm-pkg-client transitive 0.15 dep"
  - "Used wasm_layer.data.to_vec() since oci-client 0.16 uses Bytes type, not Vec<u8> directly"

patterns-established:
  - "OCI URI scheme: oci://registry/repo:tag[@sha256:digest] parsed into oci_client::Reference"
  - "OCI auth pattern: WAVS_OCI_USERNAME + WAVS_OCI_PASSWORD env vars, fallback to anonymous"

requirements-completed: [OCI-01, OCI-02, OCI-05, OCI-06]

# Metrics
duration: 21min
completed: 2026-03-24
---

# Phase 1 Plan 1: OCI Component Pull Types and Puller Summary

**ComponentSource::Oci variant with optional digest and OCI puller module using oci-client 0.16 / oci-wasm 0.4 for authenticated WASM component pulls**

## Performance

- **Duration:** 21 min
- **Started:** 2026-03-24T19:58:57Z
- **Completed:** 2026-03-24T20:19:57Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added ComponentSource::Oci variant with uri: String and digest: Option<ComponentDigest> to service.rs
- Changed digest() method to return Option<&ComponentDigest> across all variants
- Created full OCI puller module with URI parsing, authenticated pull, and env-based auth
- Added oci-client 0.16 and oci-wasm 0.4 as workspace dependencies
- All 5 unit tests pass (URI parsing with tag, digest, tag+digest, rejection, anonymous auth)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ComponentSource::Oci variant and change digest() to Option** - `ae422455` (feat)
2. **Task 2: Create OCI puller module with URI parsing and authenticated pull** - `9befd4da` (feat)

## Files Created/Modified
- `Cargo.toml` - Added oci-client 0.16 and oci-wasm 0.4 workspace dependencies
- `packages/types/src/service.rs` - Added Oci variant to ComponentSource, changed digest() to return Option
- `packages/utils/src/oci.rs` - New OCI puller module with OciUri parser, OciPuller client, auth_from_env(), 5 unit tests
- `packages/utils/src/lib.rs` - Added pub mod oci re-export
- `packages/utils/Cargo.toml` - Added oci-client and oci-wasm deps

## Decisions Made
- digest() returns Option<&ComponentDigest> to support tag-only OCI pulls where no content digest is available upfront
- OciPuller returns Vec<u8> only, never exposing oci-client types publicly, to avoid version conflicts with wasm-pkg-client's transitive oci-client 0.15
- Used .to_vec() conversion on ImageLayer.data since oci-client 0.16 uses Bytes type

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed Bytes-to-Vec<u8> type mismatch in OCI pull return**
- **Found during:** Task 2 (OCI puller module)
- **Issue:** oci-client 0.16 ImageLayer.data is Bytes (from bytes crate), not Vec<u8> as the plan assumed
- **Fix:** Added .to_vec() conversion on wasm_layer.data
- **Files modified:** packages/utils/src/oci.rs
- **Verification:** cargo check -p utils passes, all tests pass
- **Committed in:** 9befd4da (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial type conversion fix. No scope creep.

## Issues Encountered
None - the Bytes type mismatch was the only surprise, immediately resolved.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ComponentSource::Oci variant and OCI puller are ready for Plan 02 to wire into the engine
- Plan 02 will fix ~8 call sites broken by digest() returning Option
- Plan 02 will add OCI fetch path to the component resolution logic in the engine

## Self-Check: PASSED

- FOUND: packages/utils/src/oci.rs
- FOUND: packages/types/src/service.rs
- FOUND: 01-01-SUMMARY.md
- FOUND: ae422455
- FOUND: 9befd4da

---
*Phase: 01-oci-component-pull*
*Completed: 2026-03-24*
