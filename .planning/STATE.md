---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: planning
stopped_at: Phase 5 context gathered
last_updated: "2026-03-19T16:34:03.545Z"
last_activity: 2026-03-18 -- Roadmap created for v1.1 BLS Signatures milestone
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 5 - BLS Types and Key Derivation

## Current Position

Phase: 5 of 8 (BLS Types and Key Derivation)
Plan: Not started
Status: Ready to plan
Last activity: 2026-03-18 -- Roadmap created for v1.1 BLS Signatures milestone

Progress: [##########..........] 50% (4/8 phases, v1.0 complete)

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

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-19T16:34:03.541Z
Stopped at: Phase 5 context gathered
Resume file: .planning/phases/05-bls-types-and-key-derivation/05-CONTEXT.md
