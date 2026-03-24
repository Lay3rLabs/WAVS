# WAVS

## What This Is

WAVS (WebAssembly-based Actively Validated Services) is a platform for running decentralized off-chain computation anchored to blockchains. Operators run sandboxed WASM components, reach multi-operator consensus via P2P (commonware), and submit verified results on-chain. Services declare their own trigger, signature scheme (secp256k1 or BLS12-381), and submission target.

## Core Value

Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain.

## Current Milestone: v1.2 Tauri App

**Goal:** Bring the Tauri desktop app up to date with v1.0/v1.1 backend features — BLS service deployment with operator registration, full P2P/operator visibility, unified activity events, and settings UX overhaul.

**Target features:**
- BLS/ECDSA algorithm selector in service builder with BLS operator key registration flow
- P2P page: connected peers, Ed25519 identity, BLS/ECDSA key display, quorum progress per service
- Unified event cards in Activity (trigger + submission result merged, error display)
- Settings page reorganization and visual polish

## Requirements

### Validated

- ✓ Dispatcher orchestrates four subsystems (trigger, engine, submission, aggregator) via crossbeam channels — existing
- ✓ Engine executes WASM components in sandboxed Wasmtime environments — existing
- ✓ SubmissionManager signs operator results with derived ECDSA keys — existing
- ✓ Aggregator collects submissions, manages quorum queues, submits on-chain (EVM + Cosmos) — existing
- ✓ HTTP API server (Axum) exposes service management, health checks, P2P status — existing
- ✓ Service registry tracks registered services and workflows — existing
- ✓ Deterministic P2P identity derived from signing mnemonic — existing
- ✓ Per-service topic isolation for P2P message routing — existing
- ✓ Catch-up protocol for missed messages when peers reconnect — existing
- ✓ Configurable discovery modes (local dev, remote production) — existing
- ✓ `/p2p/status` endpoint exposes peer info, topics, mesh counts — existing
- ✓ Quorum queue with automatic retry for transient errors — existing
- ✓ Replace libp2p with commonware-p2p for authenticated peer communication — v1.0
- ✓ Replace GossipSub with commonware-broadcast for per-service message dissemination — v1.0
- ✓ Replace libp2p identity (secp256k1) with commonware-cryptography (Ed25519) — v1.0
- ✓ Use commonware-p2p discovery mode (bootstrapper-based) for peer discovery — v1.0
- ✓ Per-service message isolation using application-level ServiceRouter filtering — v1.0
- ✓ Catch-up / message caching via commonware-broadcast buffered Engine — v1.0
- ✓ New P2P config format tailored to commonware (Disabled / Local / Remote) — v1.0
- ✓ Updated `/p2p/status` endpoint with Ed25519 peer ID, socket addresses, connected peers — v1.0
- ✓ Updated P2P documentation in `docs/P2P.md` with commonware setup instructions — v1.0
- ✓ Blog post in `docs/blog/` announcing the commonware integration — v1.0
- ✓ All existing e2e tests pass with commonware P2P backend — v1.0
- ✓ libp2p dependency removed from Cargo.toml — v1.0
- ✓ `SignatureAlgorithm::Bls12381` variant in Rust types and WIT interface — v1.1
- ✓ BLS submission type: G2 aggregate sig + sorted G1 signer pubkeys + reference block — v1.1
- ✓ poa-middleware BLS contract ABIs imported into `packages/types` — v1.1
- ✓ BLS private key derived deterministically from signing mnemonic per service (blst crate) — v1.1
- ✓ BLS public key (G1, 128 bytes) derivable from private key for operator registration — v1.1
- ✓ Operator signs envelope with BLS key → G2 signature via hash-to-curve (consistent with HashToCurve.sol) — v1.1
- ✓ BLS signature + G1 pubkey propagated in Submission over P2P — v1.1
- ✓ Secp256k1 signing path unchanged for secp256k1 services — v1.1
- ✓ Aggregator accumulates BLS sigs and pubkeys until quorum, aggregates G2 + G1, captures referenceBlock — v1.1
- ✓ Aggregated SignatureData submitted to BLS service manager contract — v1.1
- ✓ E2E test: BLS service on local anvil with poa-middleware BLS contracts, multi-operator quorum — v1.1
- ✓ Existing secp256k1 e2e tests unchanged and passing — v1.1

### Active

(Defined in REQUIREMENTS.md for v1.2)

### Out of Scope

- MCP tooling updates for BLS operator registration — manual registration for now
- Threshold/DKG signatures (commonware threshold-simplex) — foundational BLS first, threshold later
- Cosmos submission with BLS — EVM only for now
- Trigger and engine subsystem changes — unaffected by signature scheme

## Context

v1.0 shipped (2026-03-18): commonware-p2p backend, Ed25519 P2P identity, libp2p fully removed.
v1.1 shipped (2026-03-23): BLS12-381 submission signatures, off-chain aggregation, poa-middleware BLS contract integration, E2E verified.
Tech stack: Rust 1.91, Wasmtime, Alloy, commonware (p2p + broadcast + cryptography), blst 0.3.16, poa-middleware BLS contracts.
+9,362 / -550 lines across 122 files in v1.1. Both secp256k1 and BLS paths fully tested.

## Constraints

- **Coexistence**: secp256k1 and bls12381 work as per-service options — no breaking changes to existing services
- **Runtime**: blst signing is sync/CPU-bound — runs on blocking thread pool (spawn_blocking)
- **Hash-to-curve**: hash-to-curve matches `HashToCurve.sol` (RFC 9380, DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`)
- **Pubkey sort**: aggregator sorts signerPubkeys by keccak256(pubkey) ascending — contract enforces this
- **Reference block**: referenceBlock < current block at submission, >= block when operators registered keys

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use commonware-p2p discovery mode | Bootstrapper-based discovery closest to Kademlia, works for dev and prod | ✓ Good |
| Single broadcast channel with ServiceRouter filtering | Simpler than per-service channels; same isolation | ✓ Good |
| Ed25519 for P2P identity | Commonware's native crypto, cleaner than wrapping secp256k1 | ✓ Good |
| Clean break on config format | Simpler than compat layer for networking rewrite | ✓ Good |
| rand_chacha 0.3 (not 0.9) | commonware-cryptography depends on rand_core 0.6 | ✓ Required |
| HKDF-SHA256 for BLS key derivation | Deterministic from mnemonic+HD index, domain-separated (WAVS-BLS-KEY-v1) | ✓ Good — consistent keys across restarts |
| blst directly (not commonware Signer::sign) | Contract-compatible DST requires RO suffix, not POP | ✓ Required — DST mismatch would break on-chain verification |
| SignatureData/WavsSignature/WavsCryptoSigner as enums | Clean BLS/secp256k1 dispatch, pattern matching for algorithm paths | ✓ Good — extensible for future algorithms |
| WavsSignature tagged serde | Breaking serialization change from old struct, but cleaner wire format | ✓ Good — clean migration |
| Per-test middleware dispatch (EvmMiddlewares) | BLS and secp256k1 tests need different contract stacks | ✓ Good — enables mixed-mode testing |
| SimpleBlsSubmit (not ISimpleSubmit) | BLS and ECDSA SignatureData are incompatible types | ✓ Required — separate contract |
| Prague anvil default (no --hardfork flag) | anvil 1.4.4 defaults to Prague; flag was redundant | ✓ Good — simpler config |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-25 after Phase 13 complete — SignaturePrefix type unified, BLS guidance banner added*
