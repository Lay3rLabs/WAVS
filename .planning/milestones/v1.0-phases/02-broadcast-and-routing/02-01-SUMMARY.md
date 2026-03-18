---
phase: 02-broadcast-and-routing
plan: 01
subsystem: p2p
tags: [commonware-broadcast, codec, digestible, sha256, service-router, retry-queue, p2p-message]

# Dependency graph
requires:
  - phase: 01-secure-peer-connectivity
    provides: Ed25519 identity, commonware-p2p networking, Oracle authorization, broadcast channel registration
provides:
  - P2pMessage struct with Codec (Write+EncodeSize+Read) and Digestible traits
  - ServiceRouter for per-service message filtering
  - RetryQueue for bounded failed-publish buffering
  - SHA-256 digest-based deduplication foundation (BCAST-02)
affects: [02-broadcast-and-routing plan 02, broadcast Engine integration, P2pHandle wiring]

# Tech tracking
tech-stack:
  added: [commonware-broadcast]
  patterns: [Codec trait implementation for custom types, Digestible for dedup, application-level message filtering]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs
    - packages/wavs/Cargo.toml

key-decisions:
  - "P2pMessage uses [u8; 32] for service_id_bytes (not Vec<u8>) for zero-cost comparison with ServiceId::inner()"
  - "P2pMessage::Read Cfg is (RangeCfg<usize>, ()) to enable ReadRangeExt ergonomic API"
  - "ServiceRouter uses HashSet<[u8; 32]> for O(1) lookup on raw service ID bytes"
  - "RetryQueue bounded at 64 items with oldest-drop eviction (BCAST-04)"
  - "Digestible impl concatenates service_id_bytes + payload before SHA-256 hashing for deterministic dedup digest"

patterns-established:
  - "Codec impl pattern: fixed bytes written raw (no length prefix), variable bytes use Vec<u8> codec (length-prefixed)"
  - "Mock Submission construction for tests via alloy_primitives types and wavs_types builders"

requirements-completed: [BCAST-02, BCAST-03, BCAST-04, BCAST-05]

# Metrics
duration: 22min
completed: 2026-03-17
---

# Phase 2 Plan 1: P2pMessage with Codec+Digestible, ServiceRouter, and RetryQueue Summary

**P2pMessage envelope with SHA-256 digest dedup, service-ID filtering router, and bounded retry queue -- all building blocks for broadcast Engine integration**

## Performance

- **Duration:** 22 min
- **Started:** 2026-03-17T16:54:32Z
- **Completed:** 2026-03-17T17:17:04Z
- **Tasks:** 3 (Task 0: test stubs, Task 1: P2pMessage impl, Task 2: ServiceRouter + RetryQueue)
- **Files modified:** 3 (p2p.rs, Cargo.toml, Cargo.lock)

## Accomplishments
- P2pMessage implements Codec (Write+EncodeSize+Read) and Digestible traits with SHA-256 digest for broadcast Engine dedup (BCAST-02)
- P2pMessage round-trips ServiceId + Submission through from_submission/to_submission conversion
- ServiceRouter filters inbound P2pMessages by subscribed service IDs using HashSet<[u8; 32]> (BCAST-05)
- RetryQueue stores up to 64 failed-publish messages with FIFO drain and oldest-drop eviction (BCAST-04)
- commonware-broadcast dependency added for Phase 2 Engine integration
- All 12 unit tests passing (4 P2pMessage + 4 ServiceRouter + 4 RetryQueue)

## Task Commits

Each task was committed atomically:

1. **Task 0: Create failing test stubs** - `bd5659a2` (test)
2. **Task 1+2: Implement P2pMessage, ServiceRouter, RetryQueue + fill test bodies** - `11783186` (feat)
3. **Cargo.lock update** - `65db0af8` (chore)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Added P2pMessage (Codec+Digestible), ServiceRouter, RetryQueue, and 12 unit tests
- `packages/wavs/Cargo.toml` - Added commonware-broadcast = "2026.3.0" dependency
- `Cargo.lock` - Updated lockfile for new dependency

## Decisions Made
- Used `[u8; 32]` for P2pMessage::service_id_bytes and ServiceRouter's HashSet key -- matches ServiceId::inner() without allocation, enables zero-cost comparison
- P2pMessage::Read Cfg is `(RangeCfg<usize>, ())` tuple to satisfy ReadRangeExt trait bounds for ergonomic deserialization
- Digestible impl concatenates service_id_bytes + payload into a single buffer before SHA-256 hashing -- deterministic and order-preserving
- Combined Tasks 1 and 2 into a single implementation commit since all types share the same file and test module

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed bytes crate import path**
- **Found during:** Task 1 (P2pMessage Codec implementation)
- **Issue:** `use bytes::{Buf, BufMut}` failed -- `bytes` crate is not a direct dependency of wavs
- **Fix:** Changed import to `use commonware_runtime::{Buf, BufMut}` which re-exports the bytes crate types
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** `cargo check -p wavs` passes
- **Committed in:** 11783186 (Task 1+2 commit)

**2. [Rule 3 - Blocking] Fixed P2pMessage Read Cfg type for ReadRangeExt compatibility**
- **Found during:** Task 1 (P2pMessage Codec tests)
- **Issue:** P2pMessage::Cfg was `RangeCfg<usize>` but ReadRangeExt requires `(RangeCfg<usize>, X)` tuple
- **Fix:** Changed Cfg to `(RangeCfg<usize>, ())` and updated read_cfg signature to destructure the tuple
- **Files modified:** packages/wavs/src/subsystems/aggregator/p2p.rs
- **Verification:** `cargo test -p wavs --lib -- p2p_broadcast_tests::test_p2p_message_codec_roundtrip` passes
- **Committed in:** 11783186 (Task 1+2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary for correct compilation and API ergonomics. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- P2pMessage, ServiceRouter, and RetryQueue are ready for Plan 02's broadcast Engine integration
- P2pMessage implements all traits required by commonware-broadcast's Engine (Codec + Digestible)
- ServiceRouter provides the filtering logic for the bridge loop's incoming message handler
- RetryQueue provides the retry buffer for failed publishes when no peers are connected

## Self-Check: PASSED

All files exist, all commits verified, all 12 tests pass.

---
*Phase: 02-broadcast-and-routing*
*Completed: 2026-03-17*
