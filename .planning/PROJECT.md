# WAVS

## What This Is

WAVS (WebAssembly-based Actively Validated Services) is a platform for running decentralized off-chain computation anchored to blockchains. Operators run sandboxed WASM components, reach multi-operator consensus via P2P (commonware), and submit verified results on-chain. Services declare their own trigger, signature scheme, and submission target.

## Core Value

Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain.

## Current Milestone: v1.1 BLS Signatures

**Goal:** Add BLS12-381 as a per-service signature scheme alongside secp256k1 — operators sign submissions with BLS keys, the aggregator combines signatures off-chain into a single aggregate, and submissions are verified by the poa-middleware BLS service manager contracts.

**Target features:**
- BLS12-381 signing in the submission pipeline (blst crate, hash-to-curve consistent with contracts)
- Off-chain BLS aggregation in the aggregator (G2 sig + G1 pubkey accumulation → single aggregate)
- poa-middleware BLS contract ABI integration and on-chain verification
- Per-service algorithm config: secp256k1 and bls12381 coexist, existing services unchanged

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

### Active

- [ ] `SignatureAlgorithm::Bls12381` variant in Rust types and WIT interface — v1.1
- [ ] BLS submission type: G2 aggregate sig + sorted G1 signer pubkeys + reference block — v1.1
- [ ] poa-middleware BLS contract ABIs imported into `packages/types` — v1.1
- [ ] BLS private key derived deterministically from signing mnemonic per service (blst crate) — v1.1
- [ ] BLS public key (G1, 128 bytes) derivable from private key for operator registration — v1.1
- [ ] Operator signs envelope with BLS key → G2 signature via hash-to-curve (consistent with HashToCurve.sol) — v1.1
- [ ] BLS signature + G1 pubkey propagated in Submission over P2P — v1.1
- [ ] Secp256k1 signing path unchanged for secp256k1 services — v1.1
- [ ] Aggregator accumulates BLS sigs and pubkeys until quorum, aggregates G2 + G1, captures referenceBlock — v1.1
- [ ] Aggregated SignatureData submitted to BLS service manager contract — v1.1
- [ ] E2E test: BLS service on local anvil with poa-middleware BLS contracts, multi-operator quorum — v1.1
- [ ] Existing secp256k1 e2e tests unchanged and passing — v1.1

### Out of Scope

- MCP tooling updates for BLS operator registration — manual registration for now, defer to v1.2
- Tauri desktop app changes — backend signature scheme transparent to frontend
- Threshold/DKG signatures (commonware threshold-simplex) — foundational BLS first, threshold later
- Cosmos submission with BLS — EVM only for this milestone
- Trigger and engine subsystem changes — unaffected by signature scheme

## Context

v1.0 shipped: commonware-p2p backend, Ed25519 P2P identity, libp2p fully removed.
v1.1 target: BLS12-381 submission signatures, off-chain aggregation, poa-middleware integration.
Tech stack additions: `blst` 0.3.16 (already transitive dep via commonware-cryptography), poa-middleware BLS contracts at `contracts/poa-middleware/`.
EIP-2537 precompiles (Pectra) used by BLS contracts for on-chain pairing verification.

## Constraints

- **Coexistence**: secp256k1 and bls12381 must work as per-service options — no breaking changes to existing services
- **Runtime**: blst signing is sync/CPU-bound — must not block Tokio async runtime (run on blocking thread pool)
- **Hash-to-curve**: WAVS hash-to-curve implementation must match `HashToCurve.sol` (RFC 9380, DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`) for on-chain verification
- **Pubkey sort**: aggregator must sort signerPubkeys by keccak256(pubkey) ascending — contract enforces this
- **Reference block**: referenceBlock must be < current block at submission time, >= block when operators registered keys
- **Testing**: E2e tests in `packages/layer-tests/` must pass — both secp256k1 and BLS paths

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use commonware-p2p discovery mode | Bootstrapper-based discovery is closest to current Kademlia model, works for both dev and prod | ✓ Good — worked cleanly for both modes |
| Single broadcast channel with ServiceRouter app-level filtering | Simpler than per-service channels; ServiceRouter achieves same isolation with less complexity | ✓ Good — clean implementation, BCAST-05 satisfied |
| Ed25519 for P2P identity | Commonware's native crypto scheme, cleaner integration than wrapping secp256k1 | ✓ Good — BIP-39 + ChaCha20Rng seeding pattern established |
| Clean break on config format | Major version change, simpler than maintaining compat layer for a networking rewrite | ✓ Good — Disabled/Local/Remote format is clean and operator-friendly |
| Announcement-style blog post | Focus on why we switched and what it means for operators, not a deep technical tutorial | ✓ Good — completed in Phase 4 |
| rand_chacha 0.3 (not 0.9) | commonware-cryptography depends on rand_core 0.6; 0.9 causes trait mismatch | ✓ Required — version pinning documented |

---
*Last updated: 2026-03-18 after v1.1 milestone start*
