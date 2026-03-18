---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: BLS Signatures
status: in_progress
stopped_at: defining requirements
last_updated: "2026-03-18T00:00:00.000Z"
last_activity: 2026-03-18 — Milestone v1.1 started
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-18)

**Core value:** Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase not started — defining requirements

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-18 — Milestone v1.1 BLS Signatures started

## Accumulated Context

### Decisions

- BLS coexists with secp256k1 as a per-service option — no breaking changes to existing services
- Off-chain BLS aggregation in WAVS aggregator — one aggregate sig per submission (cheaper gas, simpler contract)
- No MCP tooling for BLS in this milestone — operators register manually, defer to v1.2
- blst 0.3.16 already in Cargo.lock as transitive dep via commonware-cryptography — no new dep needed
- poa-middleware BLS contracts are the target: POAStakeRegistry.sol + BLS12381.sol + HashToCurve.sol
- EIP-2537 precompiles (Pectra) used on-chain for pairing verification
- Hash-to-curve DST must match: BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_
- signerPubkeys must be sorted by keccak256(pubkey) ascending — contract enforces this
- referenceBlock must be < current block at submission time

### Pending Todos

None yet.

### Blockers/Concerns

None yet.
