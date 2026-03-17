# Roadmap: WAVS Commonware P2P Migration

## Overview

This roadmap replaces the WAVS aggregator's libp2p networking layer with commonware primitives across four phases. Phase 1 establishes secure peer connectivity (identity, networking, security). Phase 2 wires broadcast, catch-up, and the P2pHandle command surface so operators can exchange signed submissions. Phase 3 delivers the new config format and observability endpoints so operators can configure and monitor their nodes. Phase 4 validates the complete migration with e2e tests, removes libp2p, and publishes documentation.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Secure Peer Connectivity** - Ed25519 identity, commonware-p2p networking, and Oracle-based security
- [ ] **Phase 2: Broadcast and Routing** - Message dissemination, catch-up, service filtering, and P2pHandle reimplementation
- [ ] **Phase 3: Config and Observability** - New P2P config format, dev presets, and updated status endpoint
- [ ] **Phase 4: Validation and Cleanup** - E2E tests, libp2p removal, documentation, and blog post

## Phase Details

### Phase 1: Secure Peer Connectivity
**Goal**: Two WAVS nodes can discover each other, establish authenticated encrypted connections using Ed25519 identities derived from their mnemonics, and enforce operator authorization
**Depends on**: Nothing (first phase)
**Requirements**: IDEN-01, IDEN-02, NET-01, NET-02, NET-03, NET-04, SEC-01, SEC-02, SEC-03
**Success Criteria** (what must be TRUE):
  1. An Ed25519 keypair derived from a known mnemonic produces the same peer ID on every invocation (deterministic identity)
  2. Two WAVS nodes started with discovery mode (bootstrappers) find each other and establish a connection
  3. Two WAVS nodes started with lookup mode (known addresses) connect to each other on localhost
  4. A node whose peer ID is not in the Oracle's authorized set is rejected at the connection level
  5. Commonware's Runner runs on a dedicated OS thread without panicking inside WAVS's existing Tokio runtime
**Plans**: 3 plans

Plans:
- [ ] 01-01-PLAN.md — Dependencies, Ed25519 identity derivation, and P2pConfig rewrite
- [ ] 01-02-PLAN.md — Commonware runtime scaffold, lookup-mode networking, and Oracle authorization
- [ ] 01-03-PLAN.md — Discovery-mode networking, block-peer API, and integration tests

### Phase 2: Broadcast and Routing
**Goal**: Operators can broadcast signed submissions, receive messages filtered by subscribed services, and catch up on missed messages after reconnection — all behind the existing P2pHandle API
**Depends on**: Phase 1
**Requirements**: BCAST-01, BCAST-02, BCAST-03, BCAST-04, BCAST-05, CATCH-01, CATCH-02, INT-01
**Success Criteria** (what must be TRUE):
  1. An operator broadcasting a signed submission sees it delivered to all connected peers
  2. An operator subscribed to service X receives only messages for service X (not service Y)
  3. An operator that disconnects and reconnects retrieves missed submissions via the buffered Engine's digest-based caching
  4. The P2pHandle API (publish, subscribe, unsubscribe, get_status) works identically from the Aggregator's perspective — no changes to AggregatorCommand handling
  5. Failed publishes (no connected peers) are retried from a bounded queue when peers reconnect
**Plans**: TBD

Plans:
- [ ] 02-01: TBD
- [ ] 02-02: TBD
- [ ] 02-03: TBD

### Phase 3: Config and Observability
**Goal**: Operators can configure their WAVS node's P2P layer via the new commonware-tailored config format and monitor peer state through the updated status endpoint
**Depends on**: Phase 2
**Requirements**: CFG-01, CFG-02, CFG-03, OBS-01, OBS-02
**Success Criteria** (what must be TRUE):
  1. A node started with `wavs.toml` containing the new P2P config (Disabled / Local / Remote) initializes the correct commonware mode
  2. The Local dev preset allows multi-operator testing on localhost with minimal config (just peer addresses and ports)
  3. `/p2p/status` returns peer ID (Ed25519 public key), listen addresses (socket format), connected peers, and subscribed services
**Plans**: TBD

Plans:
- [ ] 03-01: TBD

### Phase 4: Validation and Cleanup
**Goal**: The migration is complete — all e2e tests pass with the commonware backend, libp2p is fully removed, and operators have documentation for the upgrade
**Depends on**: Phase 3
**Requirements**: INT-02, INT-03, DOC-01, DOC-02, DOC-03
**Success Criteria** (what must be TRUE):
  1. `just test-wavs-e2e` passes including multi-operator scenarios
  2. libp2p and all 13 of its feature flags are removed from Cargo.toml (zero libp2p references in the dependency tree)
  3. `docs/P2P.md` documents commonware setup, config examples, and multi-node instructions for operators
  4. A blog post in `docs/blog/` announces the commonware integration (announcement style, not tutorial)
  5. An operator migration guide documents the identity change, config format change, and coordinated upgrade requirement
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Secure Peer Connectivity | 0/3 | Not started | - |
| 2. Broadcast and Routing | 0/3 | Not started | - |
| 3. Config and Observability | 0/1 | Not started | - |
| 4. Validation and Cleanup | 0/2 | Not started | - |
