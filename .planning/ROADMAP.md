# Roadmap: WAVS

## Milestones

- ✅ **v1.0 Commonware P2P Migration** -- Phases 1-4 (shipped 2026-03-18)
- ✅ **v1.1 BLS Signatures** -- Phases 5-8 (shipped 2026-03-23)
- ✅ **v1.2 Tauri App** -- Phases 9-13 (shipped 2026-03-25)
- **v1.3 Per-Service P2P Targeting** -- Phases 14-17 (in progress)

## Phases

<details>
<summary>v1.0 Commonware P2P Migration (Phases 1-4) -- SHIPPED 2026-03-18</summary>

- [x] Phase 1: Secure Peer Connectivity (3/3 plans) -- completed 2026-03-17
- [x] Phase 2: Broadcast and Routing (2/2 plans) -- completed 2026-03-17
- [x] Phase 3: Config and Observability (4/4 plans) -- completed 2026-03-17
- [x] Phase 4: Validation and Cleanup (2/2 plans) -- completed 2026-03-17

</details>

<details>
<summary>v1.1 BLS Signatures (Phases 5-8) -- SHIPPED 2026-03-23</summary>

- [x] Phase 5: BLS Types and Key Derivation (3/3 plans) -- completed 2026-03-19
- [x] Phase 6: BLS Signing Pipeline (2/2 plans) -- completed 2026-03-20
- [x] Phase 7: BLS Aggregation (2/2 plans) -- completed 2026-03-20
- [x] Phase 8: Integration and Verification (2/2 plans) -- completed 2026-03-23

</details>

<details>
<summary>v1.2 Tauri App (Phases 9-13) -- SHIPPED 2026-03-25</summary>

- [x] Phase 9: Foundation Types and Settings Refactor (2/2 plans) -- completed 2026-03-24
- [x] Phase 10: P2P Operator Dashboard (2/2 plans) -- completed 2026-03-24
- [x] Phase 11: BLS Service Builder and Registration (2/2 plans) -- completed 2026-03-24
- [x] Phase 12: Unified Activity Events (2/2 plans) -- completed 2026-03-24
- [x] Phase 13: BLS Registration UX and Type Cleanup (1/1 plan) -- completed 2026-03-24

</details>

### v1.3 Per-Service P2P Targeting (In Progress)

- [x] **Phase 14: Subscription Data Structures** - PeerSubscriptionMap, SubscriptionAnnouncement type, sentinel constant, disconnect cleanup (completed 2026-04-03)
- [x] **Phase 15: Subscription Protocol** - Announce/receive lifecycle wired into bridge loops with heartbeat sync and backward compat (completed 2026-04-03)
- [x] **Phase 16: Targeted Delivery** - Replace Recipients::All with Recipients::Some on direct channel with fallback and retry re-resolution (completed 2026-04-03)
- [ ] **Phase 17: Subscription Observability** - Per-service peer counts in /p2p/status endpoint

## Phase Details

### Phase 14: Subscription Data Structures
**Goal**: Subscription tracking infrastructure exists with tested data structures and wire format, ready for protocol integration
**Depends on**: Phase 13 (v1.2 complete)
**Requirements**: SUB-01, SUB-02, SUB-03, ANN-05
**Success Criteria** (what must be TRUE):
  1. `PeerSubscriptionMap` maintains a forward index (`service_id -> Set<PeerPubkey>`) that returns the correct peer set for any service
  2. `PeerSubscriptionMap` maintains a reverse index (`PeerPubkey -> Set<service_id>`) and removing a peer clears all its entries from both maps
  3. `SubscriptionAnnouncement` encodes/decodes via commonware-codec with a sentinel service_id (`[0xFF; 32]`) distinguishable from real service messages
  4. `get_recipients()` returns `Recipients::All` when the subscriber set for a service is empty (defensive fallback built into the data structure)
**Plans:** 1/1 plans complete
Plans:
- [x] 14-01-PLAN.md -- Subscription data structures, wire format, and comprehensive unit tests

### Phase 15: Subscription Protocol
**Goal**: Peers exchange subscription state over the existing P2P channels so every node knows which services each peer handles
**Depends on**: Phase 14
**Requirements**: ANN-01, ANN-02, ANN-03, ANN-04, COMPAT-03
**Success Criteria** (what must be TRUE):
  1. When a node adds a service, connected peers receive a subscription announcement and update their local peer subscription map
  2. When a node removes a service, connected peers receive an unsubscribe announcement and remove that service from the peer's entry
  3. Subscription state is re-broadcast on every heartbeat cycle so peers that missed an announcement eventually converge
  4. When a new peer connects, the node sends its full subscription set as a hello message
  5. A peer that has never sent any subscription announcement is treated as subscribed to all services (backward compatible with pre-v1.3 nodes)
**Plans:** 2/2 plans complete
Plans:
- [x] 15-01-PLAN.md -- Extend data structures with full_state field, set_peer_subscriptions, has_announced, and unit tests
- [x] 15-02-PLAN.md -- Wire subscription protocol into both bridge loops (subscribe/unsubscribe, inbound, heartbeat, hello)

### Phase 16: Targeted Delivery
**Goal**: Submissions on the direct channel reach only peers subscribed to that service, with reliable fallback to broadcast-all
**Depends on**: Phase 15
**Requirements**: TGT-01, TGT-02, TGT-03, TGT-04, COMPAT-01, COMPAT-02
**Success Criteria** (what must be TRUE):
  1. Submissions on channel 1 (direct) use `Recipients::Some(service_peers)` to deliver only to peers subscribed to that service
  2. When the subscriber set for a service is empty or a peer has not announced, the node falls back to `Recipients::All` instead of silently dropping messages
  3. The broadcast Engine channel (channel 0) continues using `Recipients::All` unconditionally for catch-up reliability
  4. Retry queue messages re-resolve their recipient set at drain time using current subscription state, not the stale set from original send
  5. All existing secp256k1 and BLS e2e tests pass unchanged
**Plans:** 2/2 plans complete
Plans:
- [x] 16-01-PLAN.md -- Wave 0 test (test_retry_re_resolution) + targeted delivery wired into run_lookup_network bridge loop
- [x] 16-02-PLAN.md -- Targeted delivery wired into run_discovery_network bridge loop + full suite verification + clippy clean

### Phase 17: Subscription Observability
**Goal**: Operators can inspect per-service subscription state via the HTTP API for debugging quorum and delivery issues
**Depends on**: Phase 15
**Requirements**: OBS-01
**Success Criteria** (what must be TRUE):
  1. `/p2p/status` response includes a `peer_subscriptions` field showing per-service peer counts (which peers subscribe to which services)
  2. The subscription data is consistent with the live `PeerSubscriptionMap` state
**Plans:** 1 plan
Plans:
- [ ] 17-01-PLAN.md -- Add peer_subscription_counts() to PeerSubscriptionMap, wire into P2pStatus and both GetStatus handlers

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Secure Peer Connectivity | v1.0 | 3/3 | Complete | 2026-03-17 |
| 2. Broadcast and Routing | v1.0 | 2/2 | Complete | 2026-03-17 |
| 3. Config and Observability | v1.0 | 4/4 | Complete | 2026-03-17 |
| 4. Validation and Cleanup | v1.0 | 2/2 | Complete | 2026-03-17 |
| 5. BLS Types and Key Derivation | v1.1 | 3/3 | Complete | 2026-03-19 |
| 6. BLS Signing Pipeline | v1.1 | 2/2 | Complete | 2026-03-20 |
| 7. BLS Aggregation | v1.1 | 2/2 | Complete | 2026-03-20 |
| 8. Integration and Verification | v1.1 | 2/2 | Complete | 2026-03-23 |
| 9. Foundation Types and Settings Refactor | v1.2 | 2/2 | Complete | 2026-03-24 |
| 10. P2P Operator Dashboard | v1.2 | 2/2 | Complete | 2026-03-24 |
| 11. BLS Service Builder and Registration | v1.2 | 2/2 | Complete | 2026-03-24 |
| 12. Unified Activity Events | v1.2 | 2/2 | Complete | 2026-03-24 |
| 13. BLS Registration UX and Type Cleanup | v1.2 | 1/1 | Complete | 2026-03-24 |
| 14. Subscription Data Structures | v1.3 | 1/1 | Complete    | 2026-04-03 |
| 15. Subscription Protocol | v1.3 | 2/2 | Complete    | 2026-04-03 |
| 16. Targeted Delivery | v1.3 | 2/2 | Complete    | 2026-04-03 |
| 17. Subscription Observability | v1.3 | 0/1 | Not started | - |

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for v1.0 phase details.
See [milestones/v1.1-ROADMAP.md](milestones/v1.1-ROADMAP.md) for v1.1 phase details.
See [milestones/v1.2-ROADMAP.md](milestones/v1.2-ROADMAP.md) for v1.2 phase details.
