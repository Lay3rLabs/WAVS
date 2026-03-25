# WAVS

## What This Is

WAVS (WebAssembly-based Actively Validated Services) is a platform for running decentralized off-chain computation anchored to blockchains. Operators run sandboxed WASM components, reach multi-operator consensus via P2P (commonware), and submit verified results on-chain. Services declare their own trigger, signature scheme (secp256k1 or BLS12-381), and submission target. The Tauri desktop app provides a GUI for deploying services, monitoring P2P network state, and managing operator keys.

## Core Value

Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain.

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
- ✓ `SignatureAlgorithm` type in frontend includes `'bls12381'` with P2P/signer types and Tauri commands — v1.2
- ✓ Settings page decomposed from 940-line monolith into section components with sidebar nav — v1.2
- ✓ P2P page with Ed25519 identity, connected peers, subscribed services, and operator key display — v1.2
- ✓ BLS/ECDSA algorithm selector in service builder with BLS operator key registration flow — v1.2
- ✓ Unified activity event cards merging trigger and submission lifecycle with status progression — v1.2
- ✓ BLS registration guidance banner for services without POA registry — v1.2

### Active

(No active milestone — run `/gsd:new-milestone` to define next)

### Out of Scope

- MCP tooling updates for BLS operator registration — manual registration for now
- Threshold/DKG signatures (commonware threshold-simplex) — foundational BLS first, threshold later
- Cosmos submission with BLS — EVM only for now
- Trigger and engine subsystem changes — unaffected by signature scheme
- Component library migration (shadcn/Radix) — existing hand-rolled Tailwind components work
- Real-time P2P message feed — status polling is sufficient
- Mobile app — desktop-first via Tauri

## Context

v1.0 shipped (2026-03-18): commonware-p2p backend, Ed25519 P2P identity, libp2p fully removed.
v1.1 shipped (2026-03-23): BLS12-381 submission signatures, off-chain aggregation, poa-middleware BLS contract integration, E2E verified.
v1.2 shipped (2026-03-25): Tauri desktop app updated — P2P dashboard, BLS service builder with one-click registration, unified activity events, settings overhaul.
Tech stack: Rust 1.91, Wasmtime 42, Alloy 1.0, commonware (p2p + broadcast + cryptography), blst 0.3.16, Tauri 2 + React 19 + Vite 7.
+2,333 / -1,033 lines across 35 files in v1.2. Both secp256k1 and BLS paths fully tested.

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
| HKDF-SHA256 for BLS key derivation | Deterministic from mnemonic+HD index, domain-separated (WAVS-BLS-KEY-v1) | ✓ Good |
| blst directly (not commonware Signer::sign) | Contract-compatible DST requires RO suffix, not POP | ✓ Required |
| SignatureData/WavsSignature/WavsCryptoSigner as enums | Clean BLS/secp256k1 dispatch, pattern matching for algorithm paths | ✓ Good |
| WavsSignature tagged serde | Breaking serialization change from old struct, but cleaner wire format | ✓ Good |
| Per-test middleware dispatch (EvmMiddlewares) | BLS and secp256k1 tests need different contract stacks | ✓ Good |
| SimpleBlsSubmit (not ISimpleSubmit) | BLS and ECDSA SignatureData are incompatible types | ✓ Required |
| Prague anvil default (no --hardfork flag) | anvil 1.4.4 defaults to Prague; flag was redundant | ✓ Good |
| Settings decomposed into 6 section components | Each section owns state/effects/handlers, sticky sidebar nav | ✓ Good — v1.2 |
| Map-based correlation store for activity events | Deterministic correlationKey for O(1) trigger-to-submission matching | ✓ Good — v1.2 |
| BLS state lifted to ServiceDetailPage | Enables Register button in actions bar, shared across sub-sections | ✓ Good — v1.2 |
| Registration checks on mount + manual refresh only | Avoids expensive on-chain reads on 15s poll cycle | ✓ Good — v1.2 |

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
*Last updated: 2026-03-25 after v1.2 milestone*
