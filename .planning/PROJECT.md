# WAVS Commonware P2P Migration

## What This Is

A completed refactor of the WAVS aggregator P2P networking layer, replacing libp2p with commonware primitives (`commonware-p2p`, `commonware-broadcast`, `commonware-cryptography`). The migration ships Ed25519 peer identity, bootstrapper-based discovery, broadcast with catch-up, per-service message filtering, a new config format, observability endpoint, and complete operator documentation including a blog post and migration guide.

## Core Value

Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain — using commonware instead of libp2p.

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

(none — start `/gsd:new-milestone` for next milestone requirements)

### Out of Scope

- Consensus protocol changes — aggregator quorum logic and on-chain submission remain unchanged
- EVM/Cosmos submission refactor — only the P2P networking layer changes
- Desktop app (Tauri) changes — backend P2P is transparent to the frontend
- Trigger subsystem changes — trigger distribution is unaffected
- Engine subsystem changes — WASM execution is unaffected
- SubmissionManager changes — signing logic stays the same (ECDSA for on-chain, Ed25519 only for P2P identity)
- Hyperswarm/Hypercore — separate concern, not part of this migration

## Context

Shipped v1.0 with ~1,100 lines of Rust (p2p.rs reduced from 1,839 to ~1,100 lines with full commonware backend).
Tech stack: commonware-p2p 2026.3.0, commonware-broadcast, commonware-cryptography, commonware-runtime, commonware-math.
libp2p 0.56 fully removed from workspace.

## Constraints

- **Compatibility**: On-chain contracts expect ECDSA signatures — Ed25519 is only for P2P identity, not on-chain signing
- **Runtime**: Must integrate with existing Tokio 1.47 async runtime — commonware-runtime runs on dedicated OS thread
- **Config**: Clean break on P2P config format, but `wavs.toml` structure for non-P2P sections stays the same
- **Testing**: E2e tests in `packages/layer-tests/` must pass — they test the full operator flow including P2P aggregation
- **Existing API**: `P2pHandle` interface and `AggregatorCommand` enum preserved — no changes outside the aggregator

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
*Last updated: 2026-03-18 after v1.0 milestone*
