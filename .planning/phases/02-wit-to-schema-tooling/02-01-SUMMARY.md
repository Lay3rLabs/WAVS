---
phase: 02-wit-to-schema-tooling
plan: 01
subsystem: api
tags: [wasmtime, json-schema, wit, wasm, component-model, wit-parser, lru-cache]

# Dependency graph
requires:
  - phase: 01-oci-component-pull
    provides: ComponentDigest hash type for cache keys
provides:
  - wit-schema library crate with generate_schema() public API
  - Recursive WIT Type to JSON Schema conversion for all type categories
  - $defs deduplication for shared types across exported functions
  - LRU cache keyed by ComponentDigest
  - WIT source doc comment enrichment via wit-parser
affects: [02-02-PLAN, phase-03-mcp-execution]

# Tech tracking
tech-stack:
  added: [wit-parser 0.244.0]
  patterns: [two-pass type deduplication with structural fingerprinting, externally tagged variant representation]

key-files:
  created:
    - packages/wit-schema/Cargo.toml
    - packages/wit-schema/src/lib.rs
    - packages/wit-schema/src/convert.rs
    - packages/wit-schema/src/traverse.rs
    - packages/wit-schema/src/cache.rs
    - packages/wit-schema/src/docs.rs
    - packages/wit-schema/src/types.rs
  modified:
    - Cargo.toml

key-decisions:
  - "Two-pass deduplication: first pass counts type occurrences, second pass generates schemas with $ref for shared types"
  - "Structural fingerprinting for $defs keys using field/case names since wasmtime binary API does not expose WIT type names"
  - "result<T, string> output simplification: unwrap ok type as primary schema with description noting error"
  - "wit-parser 0.244.0 matches wasmtime 42.0.1 transitive dep to avoid duplicate crate versions"

patterns-established:
  - "Pattern: type_fingerprint using field/case names joined by pipe for structural dedup"
  - "Pattern: SchemaOptions struct with optional wit_path for extensible configuration"
  - "Pattern: generate_schema for one-shot, generate_schema_cached for long-running processes"

requirements-completed: [SCHEMA-01, SCHEMA-02, SCHEMA-03, SCHEMA-04, SCHEMA-05]

# Metrics
duration: 10min
completed: 2026-03-25
---

# Phase 2, Plan 01: wit-schema Library Crate Summary

**WIT-to-JSON Schema conversion library with recursive type mapping, $defs deduplication, digest-based caching, and WIT doc comment enrichment**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-25T01:10:22Z
- **Completed:** 2026-03-25T01:21:11Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Created packages/wit-schema/ library crate with complete WIT-to-JSON Schema conversion
- All WIT type categories mapped: Bool, integers, floats, Char, String, List, Record, Variant, Enum, Option, Result, Tuple, Flags
- Special cases implemented: u128 string pattern (D-03), list<u8> base64 encoding, result<T,string> output simplification
- $defs deduplication via two-pass structural fingerprinting (D-06)
- LRU cache keyed by ComponentDigest with configurable capacity (SCHEMA-05)
- WIT source doc comment enrichment using wit-parser (SCHEMA-04, D-07)
- 15 tests pass against real WAVS compiled components (echo_data, timer_aggregator, square)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create wit-schema crate with core type conversion and export traversal**
   - `6a06bfbb` (test: add failing tests for wit-schema crate - TDD RED)
   - `50210cdf` (feat: implement wit-schema core type conversion and export traversal - TDD GREEN)
2. **Task 2: Add schema cache and doc comment enrichment** - `afc10ec9` (feat)

## Files Created/Modified
- `packages/wit-schema/Cargo.toml` - Crate configuration with wasmtime, serde_json, lru, wit-parser deps
- `packages/wit-schema/src/lib.rs` - Public API: generate_schema, generate_schema_cached, SchemaOptions, SchemaCache re-exports
- `packages/wit-schema/src/convert.rs` - Recursive WIT Type -> JSON Schema conversion with all type mappings
- `packages/wit-schema/src/traverse.rs` - Component export function discovery with nested instance traversal
- `packages/wit-schema/src/cache.rs` - LRU cache keyed by ComponentDigest with Mutex wrapper
- `packages/wit-schema/src/docs.rs` - Doc comment extraction from WIT source via wit-parser
- `packages/wit-schema/src/types.rs` - SchemaOptions struct with optional wit_path
- `Cargo.toml` - Added wit-schema to workspace members and wit-parser to workspace deps

## Decisions Made
- Used two-pass deduplication: first pass counts type occurrences across all exports, second pass generates schemas with $ref pointers for types seen more than once. This avoids the Wassette approach of inlining everything.
- Structural fingerprinting for $defs keys (e.g., field names joined by pipe) since wasmtime's binary API does not expose original WIT type names for Record/Variant types.
- result<T, string> output simplification: when error type is string (the WAVS convention), the outputSchema shows the ok type as primary with a description noting the error. Avoids wrapping every output in oneOf ok/err.
- Added wit-parser 0.244.0 as a direct dependency, matching the exact version pulled transitively by wasmtime 42.0.1 to avoid duplicate crate compilation.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- echo_data and square components share the same operator world (wavs-world) and produce identical schemas since they both export only `run` with the same type signature. Fixed the "different bytes generates new schema" test to compare echo_data against timer_aggregator (aggregator world with 3 exports) instead.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- wit-schema library crate is ready for Plan 02 (CLI integration as `wavs wit-schema <component.wasm>`)
- Phase 3 MCP server can import this crate as a library dependency for auto-generated tool descriptions
- All 8 locked decisions (D-01 through D-08) are implemented and verified

## Self-Check: PASSED

All 7 created files verified present. All 3 task commits verified in git log. SUMMARY.md verified present.

---
*Phase: 02-wit-to-schema-tooling*
*Completed: 2026-03-25*
