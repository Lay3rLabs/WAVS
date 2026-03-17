# WAVS Commonware P2P Migration

## What This Is

A refactor of the WAVS aggregator P2P networking layer, replacing libp2p with commonware primitives (`commonware-p2p`, `commonware-broadcast`, `commonware-cryptography`). This improves WAVS decentralization by adopting the commonware ecosystem's authenticated peer communication and broadcast infrastructure. The project also includes updated operator documentation, a blog post announcing the integration, and verified passing e2e tests.

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

### Active

- [ ] Replace libp2p with commonware-p2p for authenticated peer communication
- [ ] Replace GossipSub with commonware-broadcast for per-service message dissemination
- [ ] Replace libp2p identity (secp256k1) with commonware-cryptography (Ed25519)
- [ ] Use commonware-p2p discovery mode (bootstrapper-based) for peer discovery
- [ ] Implement per-service channel isolation using commonware-p2p channels
- [ ] Maintain catch-up / message caching via commonware-broadcast buffered engine
- [ ] New P2P config format tailored to commonware (clean break from `[wavs.p2p] local/remote`)
- [ ] Update `/p2p/status` endpoint to reflect commonware peer state
- [ ] Update P2P documentation in `docs/P2P.md` with commonware setup instructions
- [ ] Write blog post announcing the commonware integration in `docs/blog/`
- [ ] All existing e2e tests pass with commonware P2P backend
- [ ] Remove libp2p dependency from Cargo.toml

### Out of Scope

- Consensus protocol changes — aggregator quorum logic and on-chain submission remain unchanged
- EVM/Cosmos submission refactor — only the P2P networking layer changes
- Desktop app (Tauri) changes — backend P2P is transparent to the frontend
- Trigger subsystem changes — trigger distribution is unaffected
- Engine subsystem changes — WASM execution is unaffected
- SubmissionManager changes — signing logic stays the same (ECDSA for on-chain, Ed25519 only for P2P identity)
- Hyperswarm/Hypercore removal — separate concern, not part of this migration

## Context

The current P2P layer is ~1,800 lines in `packages/wavs/src/subsystems/aggregator/p2p.rs` using libp2p 0.56 with:
- GossipSub for per-service pub/sub (topics: `wavs/{service_id}/topic/v1`)
- Kademlia DHT (prod) and mDNS (dev) for peer discovery
- Request/Response protocol for catch-up on peer reconnection
- AutoNAT + Identify for NAT traversal
- secp256k1 keypair derived from signing mnemonic at HD path m/44'/60'/0'/0/0

Commonware provides:
- `commonware-p2p::authenticated::discovery` — bootstrapper-based peer discovery (replaces Kademlia/mDNS)
- `commonware-p2p::authenticated::lookup` — address-known peer lookup (alternative for dev)
- `commonware-broadcast::buffered` — broadcast engine with message caching and digest-based retrieval (replaces GossipSub + catch-up protocol)
- `commonware-cryptography` — Ed25519 key generation and signing (replaces libp2p secp256k1 identity)

The aggregator integrates with the dispatcher via `AggregatorCommand` enum over crossbeam channels. Key commands: `Broadcast`, `Receive`, `SubscribeService`, `UnsubscribeService`. The P2P layer is behind a `P2pHandle` abstraction that the aggregator calls into.

## Constraints

- **Compatibility**: On-chain contracts expect ECDSA signatures — Ed25519 is only for P2P identity, not on-chain signing
- **Runtime**: Must integrate with existing Tokio 1.47 async runtime — commonware-runtime compatibility needed
- **Config**: Clean break on P2P config format, but `wavs.toml` structure for non-P2P sections stays the same
- **Testing**: E2e tests in `packages/layer-tests/` must pass — they test the full operator flow including P2P aggregation
- **Existing API**: `P2pHandle` interface and `AggregatorCommand` enum should remain stable to minimize changes outside the aggregator

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use commonware-p2p discovery mode | Bootstrapper-based discovery is closest to current Kademlia model, works for both dev and prod | — Pending |
| Per-service channels (not single broadcast) | Mirrors current GossipSub topic isolation, operators only receive messages for their services | — Pending |
| Ed25519 for P2P identity | Commonware's native crypto scheme, cleaner integration than wrapping secp256k1 | — Pending |
| Clean break on config format | Major version change, simpler than maintaining compat layer for a networking rewrite | — Pending |
| Announcement-style blog post | Focus on why we switched and what it means for operators, not a deep technical tutorial | — Pending |

---
*Last updated: 2026-03-17 after initialization*
