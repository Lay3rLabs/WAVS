---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: executing
stopped_at: Completed 05-01-PLAN.md
last_updated: "2026-03-19T17:30:10Z"
last_activity: 2026-03-19 -- Plan 05-01 complete (BLS ABI bindings and SignatureAlgorithm variant)
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 3
  completed_plans: 1
  percent: 54
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 5 - BLS Types and Key Derivation

## Current Position

Phase: 5 of 8 (BLS Types and Key Derivation)
Plan: 1 of 3 complete
Status: Executing phase 5
Last activity: 2026-03-19 -- Plan 05-01 complete (BLS ABI bindings and SignatureAlgorithm variant)

Progress: [###########.........] 54% (4/8 phases, v1.0 complete; plan 1/3 in phase 5)

## Performance Metrics

**Velocity:**
- Total plans completed: 11 (v1.0)
- Average duration: see v1.0 retrospective
- Total execution time: see v1.0 retrospective

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 3 | - | - |
| 2 | 2 | - | - |
| 3 | 4 | - | - |
| 4 | 2 | - | - |
| 5-01 | 1 | 11min | 11min |
| 5-8 | TBD | - | - |

*Updated after each plan completion*

## Accumulated Context

### Decisions

- BLS coexists with secp256k1 as a per-service option -- no breaking changes
- Off-chain BLS aggregation in WAVS aggregator -- one aggregate sig per submission
- blst 0.3.16 already in Cargo.lock as transitive dep via commonware-cryptography
- Hash-to-curve DST must match contracts: BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_
- signerPubkeys sorted by keccak256(pubkey) ascending -- contract enforces this
- referenceBlock must be < current block at submission time
- blst signing is CPU-bound -- must use spawn_blocking in async context
- No MCP tooling for BLS in v1.1 -- defer to v1.2
- BLS bindings use non-rpc pattern (no #[sol(rpc)]) -- BLS contract interaction handled differently in Phase 7
- BLS types re-exported as BlsServiceHandler/BlsStakeRegistry/BlsServiceManager to avoid collision with secp256k1
- SignatureKind::bls_default() uses prefix=None -- BLS uses hash-to-curve with its own DST, not EIP-191
- WavsSigner trait returns error for BLS -- dedicated BLS signer to be implemented in Plan 02

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-19T17:30:10Z
Stopped at: Completed 05-01-PLAN.md
Resume file: .planning/phases/05-bls-types-and-key-derivation/05-01-SUMMARY.md
